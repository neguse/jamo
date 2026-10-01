//! v3 ゼロコピー辞書 (`BinDict`)
//!
//! ファイルレイアウト = メモリレイアウト。辞書バイト列 (Vec または mmap) を
//! 所有し、検索時はバイト列を借用参照して必要な部分だけ読む。ロードは
//! セクション配置の検証だけで、エントリをヒープに展開しない。
//! mmap なら変換時に触れたページだけが物理メモリに載る。
//!
//! 読みは 1 バイト符号化した LOUDS trie、エントリは終端ノード順のトークン列、
//! 表層は読みと同一 (ひらがな/カタカナ) ならフラグのみ、それ以外は共有プール
//! への参照。接続行列は行デフォルト + 例外 (ビット列 + コスト)。

mod build;
mod charcode;
mod format;
#[cfg(test)]
mod tests;

use std::io;
use std::path::Path;

pub use build::{BuildInput, build};
use charcode::{ReadingCoder, SurfaceCoder, read_table};
use format::*;

use crate::bitvec::{BitRef, BitView};
use crate::dict::{Dictionary, Entry};
use crate::kana::to_katakana;
use crate::viterbi::ConnectionCost;

enum Storage {
    Owned(Vec<u8>),
    #[cfg(not(target_arch = "wasm32"))]
    Mapped(memmap2::Mmap),
}

impl Storage {
    fn bytes(&self) -> &[u8] {
        match self {
            Storage::Owned(v) => v,
            #[cfg(not(target_arch = "wasm32"))]
            Storage::Mapped(m) => m,
        }
    }
}

/// セクション別のサイズ内訳 (`jamo dict info`)
pub struct SectionInfo {
    pub name: &'static str,
    pub bytes: usize,
}

/// v3 辞書
pub struct BinDict {
    data: Storage,
    layout: Layout,
    reading_coder: ReadingCoder,
    surface_coder: SurfaceCoder,
}

impl BinDict {
    /// バイト列から開く (構造検証のみ)
    pub fn from_vec(data: Vec<u8>) -> io::Result<Self> {
        Self::new(Storage::Owned(data))
    }

    /// ファイルを mmap して開く。変換時に触れたページだけが物理メモリに載る
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        // SAFETY: 読み取り専用マップ。辞書ファイルは開いている間は書き換えない
        // 前提 (書き換えられた場合も境界検査済みの読み出しなので UB にはならず
        // 変換結果が壊れるだけ)。
        let map = unsafe { memmap2::Mmap::map(&file)? };
        // 辞書引きはランダムアクセスなので先読みを抑え、触れたページだけを載せる
        let _ = map.advise(memmap2::Advice::Random);
        Self::new(Storage::Mapped(map))
    }

    /// mmap がない環境 (WASM) ではファイル読み込みで開く
    #[cfg(target_arch = "wasm32")]
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::read(path)
    }

    /// ファイルを読み込んで開く (mmap を使わない)
    pub fn read(path: &Path) -> io::Result<Self> {
        Self::from_vec(std::fs::read(path)?)
    }

    fn new(data: Storage) -> io::Result<Self> {
        let layout = Layout::parse(data.bytes())?;
        let bytes = data.bytes();
        let reading_chars =
            read_table(&bytes[layout.sections[SEC_READING_CHARS].clone()]).map_err(invalid)?;
        if reading_chars.len() > charcode::READING_TABLE_MAX {
            return Err(invalid("reading char table too large"));
        }
        let surface_chars =
            read_table(&bytes[layout.sections[SEC_SURFACE_CHARS].clone()]).map_err(invalid)?;
        let surface_coder = SurfaceCoder::new(surface_chars).map_err(invalid)?;
        Ok(Self {
            data,
            layout,
            reading_coder: ReadingCoder::new(reading_chars),
            surface_coder,
        })
    }

    /// ファイル全体に触れる深い検証 (rank 索引、参照先の範囲)
    pub fn verify(&self) -> io::Result<()> {
        for r in [
            &self.layout.louds,
            &self.layout.terminal,
            &self.layout.entry_bounds,
            &self.layout.nonident,
            &self.layout.surface_bounds,
            &self.layout.matrix.exceptions,
        ] {
            BitView::from_ref(self.bytes(), r).verify()?;
        }
        let l = &self.layout;
        let data = self.bytes();
        let tokens = &data[l.sections[SEC_TOKENS].clone()];
        for t in 0..l.token_count {
            let v = read_u16(tokens, t * TOKEN_LEN);
            let kind = v >> 14;
            if kind > KIND_POOL || (v & 0x3FFF) as usize >= l.combo_count {
                return Err(invalid(format!("token {t} has bad kind/combo")));
            }
        }
        let ids = &data[l.sections[SEC_SURFACE_IDS].clone()];
        for i in 0..l.nonident_count {
            if self.surface_id_at(ids, i) >= l.surface_count {
                return Err(invalid(format!("surface id {i} out of range")));
            }
        }
        Ok(())
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        self.data.bytes()
    }

    #[inline]
    fn section(&self, idx: usize) -> &[u8] {
        &self.bytes()[self.layout.sections[idx].clone()]
    }

    /// 直列化済みビット列のビュー (配置は Layout::parse で検証済み)
    #[inline]
    fn bits(&self, r: &BitRef) -> BitView<'_> {
        BitView::from_ref(self.bytes(), r)
    }

    #[inline]
    fn surface_id_at(&self, ids: &[u8], i: usize) -> usize {
        let b = &ids[i * SURFACE_ID_LEN..(i + 1) * SURFACE_ID_LEN];
        u32::from_le_bytes([b[0], b[1], b[2], 0]) as usize
    }

    /// ノードの子が始まる LOUDS ビット位置
    #[inline]
    fn child_bit_start(&self, louds: &BitView<'_>, node: usize) -> usize {
        if node == 0 {
            0
        } else {
            louds
                .select0(node - 1)
                .map(|p| p + 1)
                .unwrap_or(louds.len())
        }
    }

    /// ラベル `label` の子ノードを返す
    #[inline]
    fn find_child(
        &self,
        louds: &BitView<'_>,
        labels: &[u8],
        node: usize,
        label: u8,
    ) -> Option<usize> {
        let start = self.child_bit_start(louds, node);
        let degree = louds.run_of_ones(start);
        if degree == 0 {
            return None;
        }
        let first = louds.rank1(start) + 1;
        let end = (first + degree).min(labels.len());
        labels[first.min(end)..end]
            .iter()
            .position(|&l| l == label)
            .map(|i| first + i)
    }

    /// 終端ノードのトークン範囲
    fn token_range(&self, node: usize) -> Option<std::ops::Range<usize>> {
        let terminal = self.bits(&self.layout.terminal);
        if node >= terminal.len() || !terminal.get(node) {
            return None;
        }
        let k = terminal.rank1(node);
        let bounds = self.bits(&self.layout.entry_bounds);
        let start = bounds.select1(k)?;
        let end = bounds.select1(k + 1).unwrap_or(bounds.len());
        Some(start..end.max(start))
    }

    /// 表層プールの `id` 番目を復号する
    fn surface(&self, id: usize) -> String {
        let bounds = self.bits(&self.layout.surface_bounds);
        let pool = self.section(SEC_SURFACE_POOL);
        let Some(start) = bounds.select1(id) else {
            return String::new();
        };
        let end = bounds.select1(id + 1).unwrap_or(pool.len()).min(pool.len());
        self.surface_coder.decode(&pool[start.min(end)..end])
    }

    /// トークン範囲をエントリに復号する。`reading` は恒等表層用
    fn decode_entries(&self, range: std::ops::Range<usize>, reading: &str) -> Vec<Entry> {
        let tokens = self.section(SEC_TOKENS);
        let combos = self.section(SEC_COMBOS);
        let ids = self.section(SEC_SURFACE_IDS);
        let nonident = self.bits(&self.layout.nonident);
        let mut katakana: Option<String> = None;
        let mut pool_rank = nonident.rank1(range.start.min(nonident.len()));
        let mut out = Vec::with_capacity(range.len());
        for t in range {
            if (t + 1) * TOKEN_LEN > tokens.len() {
                break;
            }
            let v = read_u16(tokens, t * TOKEN_LEN);
            let cost = read_i16(tokens, t * TOKEN_LEN + 2);
            let kind = v >> 14;
            let combo = (v & 0x3FFF) as usize;
            if (combo + 1) * COMBO_LEN > combos.len() {
                break;
            }
            let left_id = read_u16(combos, combo * COMBO_LEN);
            let right_id = read_u16(combos, combo * COMBO_LEN + 2);
            let pos_type = combos[combo * COMBO_LEN + 4];
            let surface = match kind {
                KIND_HIRAGANA => reading.to_string(),
                KIND_KATAKANA => katakana.get_or_insert_with(|| to_katakana(reading)).clone(),
                _ => {
                    let id = if pool_rank < self.layout.nonident_count {
                        self.surface_id_at(ids, pool_rank)
                    } else {
                        usize::MAX
                    };
                    pool_rank += 1;
                    self.surface(id)
                }
            };
            out.push(Entry {
                surface,
                left_id,
                right_id,
                cost,
                pos_type,
            });
        }
        out
    }

    /// 接続コスト (right_id of prev, left_id of next)
    #[inline]
    pub fn connection_cost(&self, right_id: u16, left_id: u16) -> i16 {
        let m = &self.layout.matrix;
        let (r, l) = (right_id as usize, left_id as usize);
        if r >= m.num_pos || l >= m.num_pos {
            return 0;
        }
        let idx = r * m.num_pos + l;
        let exc = self.bits(&m.exceptions);
        let data = self.bytes();
        if let Some(k) = exc.rank1_if_set(idx) {
            let off = m.costs.start + k * 2;
            if off + 2 <= m.costs.end {
                return read_i16(data, off);
            }
        }
        read_i16(data, m.defaults.start + r * 2)
    }

    pub fn vocab_size(&self) -> usize {
        self.layout.vocab_size
    }

    pub fn reading_count(&self) -> usize {
        self.layout.reading_count
    }

    pub fn pos_size(&self) -> usize {
        self.layout.matrix.num_pos
    }

    /// 辞書バイト列全体の長さ
    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes().is_empty()
    }

    /// セクション別サイズ
    pub fn sections(&self) -> Vec<SectionInfo> {
        SECTION_NAMES
            .iter()
            .zip(self.layout.sections.iter())
            .map(|(&name, r)| SectionInfo {
                name,
                bytes: r.len(),
            })
            .collect()
    }

    /// (trie ノード数, トークン数, combo 数, プール表層数, 行列例外数)
    pub fn counts(&self) -> (usize, usize, usize, usize, usize) {
        let l = &self.layout;
        (
            l.node_count,
            l.token_count,
            l.combo_count,
            l.surface_count,
            l.matrix.costs.len() / 2,
        )
    }

    /// プレフィクス検索: 指定読みで始まるエントリを返す (予測変換用)
    pub fn prefix_search(&self, prefix: &str, limit: usize) -> Vec<Entry> {
        let louds = self.bits(&self.layout.louds);
        let labels = self.section(SEC_LABELS);
        let mut node = 1;
        let mut prefix_codes = Vec::new();
        for c in prefix.chars() {
            let mut codes = Vec::new();
            self.reading_coder.encode_char(c, &mut codes);
            for &b in &codes {
                match self.find_child(&louds, labels, node, b) {
                    Some(child) => node = child,
                    None => return Vec::new(),
                }
            }
            prefix_codes.extend(codes);
        }

        let mut results = Vec::new();
        let mut stack = vec![(node, prefix_codes)];
        while let Some((n, codes)) = stack.pop() {
            if let Some(range) = self.token_range(n) {
                let reading = self.reading_coder.decode(&codes);
                results.extend(self.decode_entries(range, &reading));
                if results.len() >= limit {
                    results.truncate(limit);
                    break;
                }
            }
            let start = self.child_bit_start(&louds, n);
            let degree = louds.run_of_ones(start);
            if degree == 0 {
                continue;
            }
            let first = louds.rank1(start) + 1;
            for child in (first..(first + degree).min(labels.len())).rev() {
                let mut c = codes.clone();
                c.push(labels[child]);
                stack.push((child, c));
            }
        }
        results
    }
}

impl std::fmt::Debug for BinDict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinDict")
            .field("bytes", &self.len())
            .field("vocab", &self.layout.vocab_size)
            .field("readings", &self.layout.reading_count)
            .finish()
    }
}

impl Dictionary for BinDict {
    fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)> {
        let louds = self.bits(&self.layout.louds);
        let labels = self.section(SEC_LABELS);
        let mut results = Vec::new();
        let mut node = 1;
        let mut codes = Vec::with_capacity(4);
        'outer: for (pos, c) in input.char_indices() {
            codes.clear();
            self.reading_coder.encode_char(c, &mut codes);
            for &b in &codes {
                match self.find_child(&louds, labels, node, b) {
                    Some(child) => node = child,
                    None => break 'outer,
                }
            }
            let end = pos + c.len_utf8();
            if let Some(range) = self.token_range(node) {
                results.push((end, self.decode_entries(range, &input[..end])));
            }
        }
        results
    }
}

impl ConnectionCost for BinDict {
    fn cost(&self, right_id: u16, left_id: u16) -> i32 {
        self.connection_cost(right_id, left_id) as i32
    }
}
