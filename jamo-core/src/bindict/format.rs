//! v3 辞書ファイルのセクション配置
//!
//! ファイル先頭に固定長ヘッダと TOC (各セクションの offset/len) を置き、
//! 以降はセクション本体。全セクションは 8 バイト境界に揃える。
//! レイアウトの詳細は doc/dict-format.md を参照。
//!
//! `Layout::parse` はサイズ・個数の整合だけを検証する (ファイル全体には
//! 触れない)。内容の深い検証は `BinDict::verify` で行う。

use std::io;
use std::ops::Range;

use crate::bitvec::{BitRef, BitView};

pub const MAGIC: &[u8; 4] = b"JAMO";
pub const VERSION: u32 = 3;
pub const SECTION_COUNT: usize = 15;
pub const HEADER_LEN: usize = 16 + SECTION_COUNT * 8;

pub const SEC_READING_CHARS: usize = 0;
pub const SEC_LOUDS: usize = 1;
pub const SEC_LABELS: usize = 2;
pub const SEC_TERMINAL: usize = 3;
pub const SEC_ENTRY_BOUNDS: usize = 4;
pub const SEC_TOKENS: usize = 5;
pub const SEC_COMBOS: usize = 6;
pub const SEC_NONIDENT: usize = 7;
pub const SEC_SURFACE_IDS: usize = 8;
pub const SEC_SURFACE_CHARS: usize = 9;
pub const SEC_SURFACE_POOL: usize = 10;
pub const SEC_SURFACE_BOUNDS: usize = 11;
pub const SEC_MATRIX: usize = 12;
pub const SEC_STATS: usize = 13;
pub const SEC_BOUNDARY: usize = 14;

pub const SECTION_NAMES: [&str; SECTION_COUNT] = [
    "reading chars",
    "louds bitvec",
    "labels",
    "terminal bitvec",
    "entry bounds",
    "tokens",
    "combos",
    "nonident bitvec",
    "surface ids",
    "surface chars",
    "surface pool",
    "surface bounds",
    "conn matrix",
    "stats",
    "boundary",
];

pub const TOKEN_LEN: usize = 4;
pub const COMBO_LEN: usize = 5;
pub const SURFACE_ID_LEN: usize = 3;
/// 境界ペナルティ 1 品詞分 (u16 prefix, u16 suffix)
pub const BOUNDARY_LEN: usize = 4;
/// combo ID は 14 bit (上位 2 bit は表層種別)
pub const MAX_COMBOS: usize = 1 << 14;
pub const MAX_POS: usize = 8192;

/// 表層種別: 読みそのまま (ひらがな)
pub const KIND_HIRAGANA: u16 = 0;
/// 表層種別: 読みのカタカナ
pub const KIND_KATAKANA: u16 = 1;
/// 表層種別: 表層プール参照
pub const KIND_POOL: u16 = 2;

pub fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

pub fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

pub fn read_i16(data: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([data[off], data[off + 1]])
}

pub fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// 接続行列セクションの内部配置
#[derive(Debug, Clone)]
pub struct MatrixLayout {
    pub num_pos: usize,
    /// 行デフォルト (i16 × num_pos)
    pub defaults: Range<usize>,
    /// 例外ビット列 (num_pos² bits)
    pub exceptions: BitRef,
    /// 例外コスト (i16 × 例外数)
    pub costs: Range<usize>,
}

/// パース済みのセクション配置
#[derive(Debug, Clone)]
pub struct Layout {
    pub sections: [Range<usize>; SECTION_COUNT],
    pub louds: BitRef,
    pub terminal: BitRef,
    pub entry_bounds: BitRef,
    pub nonident: BitRef,
    pub surface_bounds: BitRef,
    pub node_count: usize,
    pub token_count: usize,
    pub combo_count: usize,
    pub nonident_count: usize,
    pub surface_count: usize,
    pub matrix: MatrixLayout,
    pub vocab_size: usize,
    pub reading_count: usize,
}

impl Layout {
    pub fn parse(data: &[u8]) -> io::Result<Self> {
        let sections = Self::parse_toc(data)?;

        let bits = |idx: usize| -> io::Result<BitRef> {
            let (r, used) = BitRef::parse(&data[sections[idx].clone()])?;
            if used != sections[idx].len() {
                return Err(invalid(format!("section {idx} has trailing bytes")));
            }
            Ok(r.offset(sections[idx].start))
        };

        // trie: ノード数 = LOUDS の 1 の数 + 1 = labels 長 = terminal 長
        let louds_ref = bits(SEC_LOUDS)?;
        let louds = BitView::from_ref(data, &louds_ref);
        let node_count = louds.count_ones() + 1;
        if louds.len() < 3 || louds.rank0(louds.len()) != node_count {
            return Err(invalid("LOUDS bit/node mismatch"));
        }
        if sections[SEC_LABELS].len() != node_count {
            return Err(invalid("labels/node count mismatch"));
        }
        let terminal_ref = bits(SEC_TERMINAL)?;
        let terminal = BitView::from_ref(data, &terminal_ref);
        if terminal.len() != node_count {
            return Err(invalid("terminal/node count mismatch"));
        }

        // トークン: 境界 BitVec と nonident BitVec はトークン数、終端数 = 境界の 1 の数
        let token_count = section_count(&sections, SEC_TOKENS, TOKEN_LEN, "tokens")?;
        let entry_bounds_ref = bits(SEC_ENTRY_BOUNDS)?;
        let entry_bounds = BitView::from_ref(data, &entry_bounds_ref);
        if entry_bounds.len() != token_count {
            return Err(invalid("entry bounds/token count mismatch"));
        }
        if entry_bounds.count_ones() != terminal.count_ones() {
            return Err(invalid("entry bounds/terminal count mismatch"));
        }
        if token_count > 0 && !entry_bounds.get(0) {
            return Err(invalid("entry bounds must start with 1"));
        }
        let combo_count = section_count(&sections, SEC_COMBOS, COMBO_LEN, "combos")?;
        if combo_count > MAX_COMBOS {
            return Err(invalid("too many combos"));
        }
        let nonident_ref = bits(SEC_NONIDENT)?;
        let nonident = BitView::from_ref(data, &nonident_ref);
        if nonident.len() != token_count {
            return Err(invalid("nonident/token count mismatch"));
        }
        let nonident_count = nonident.count_ones();
        if sections[SEC_SURFACE_IDS].len() != nonident_count * SURFACE_ID_LEN {
            return Err(invalid("surface ids/nonident count mismatch"));
        }

        // 表層プール: 境界 BitVec はプールのバイト数
        let surface_bounds_ref = bits(SEC_SURFACE_BOUNDS)?;
        let surface_bounds = BitView::from_ref(data, &surface_bounds_ref);
        if surface_bounds.len() != sections[SEC_SURFACE_POOL].len() {
            return Err(invalid("surface bounds/pool length mismatch"));
        }
        let surface_count = surface_bounds.count_ones();
        if surface_count > 0 && !surface_bounds.get(0) {
            return Err(invalid("surface bounds must start with 1"));
        }

        let matrix = Self::parse_matrix(data, sections[SEC_MATRIX].clone())?;

        let (vocab_size, reading_count) = Self::parse_stats(data, sections[SEC_STATS].clone())?;
        let boundary_len = sections[SEC_BOUNDARY].len();
        if boundary_len != 0 && boundary_len != matrix.num_pos * BOUNDARY_LEN {
            return Err(invalid("boundary section size mismatch"));
        }

        Ok(Self {
            sections,
            louds: louds_ref,
            terminal: terminal_ref,
            entry_bounds: entry_bounds_ref,
            nonident: nonident_ref,
            surface_bounds: surface_bounds_ref,
            node_count,
            token_count,
            combo_count,
            nonident_count,
            surface_count,
            matrix,
            vocab_size,
            reading_count,
        })
    }

    /// ヘッダと TOC を読み、各セクションがファイル内に収まり重ならないことを確認する
    fn parse_toc(data: &[u8]) -> io::Result<[Range<usize>; SECTION_COUNT]> {
        if data.len() < HEADER_LEN {
            return Err(invalid("file too small"));
        }
        if &data[..4] != MAGIC {
            return Err(invalid("invalid magic"));
        }
        let version = read_u32(data, 4);
        if version != VERSION {
            return Err(invalid(format!("unsupported version: {version}")));
        }
        if read_u32(data, 8) as usize != SECTION_COUNT {
            return Err(invalid("section count mismatch"));
        }
        let mut sections: [Range<usize>; SECTION_COUNT] = Default::default();
        let mut prev_end = HEADER_LEN;
        for (i, sec) in sections.iter_mut().enumerate() {
            let off = read_u32(data, 16 + i * 8) as usize;
            let len = read_u32(data, 20 + i * 8) as usize;
            let end = off
                .checked_add(len)
                .filter(|&e| e <= data.len())
                .ok_or_else(|| invalid(format!("section {i} out of file")))?;
            if off < prev_end {
                return Err(invalid(format!("section {i} overlaps")));
            }
            *sec = off..end;
            prev_end = end;
        }
        Ok(sections)
    }

    fn parse_stats(data: &[u8], sec: Range<usize>) -> io::Result<(usize, usize)> {
        if sec.len() != 8 {
            return Err(invalid("stats section size"));
        }
        Ok((
            read_u32(data, sec.start) as usize,
            read_u32(data, sec.start + 4) as usize,
        ))
    }

    fn parse_matrix(data: &[u8], sec: Range<usize>) -> io::Result<MatrixLayout> {
        if sec.len() < 4 {
            return Err(invalid("matrix section truncated"));
        }
        let num_pos = read_u32(data, sec.start) as usize;
        if num_pos == 0 || num_pos > MAX_POS {
            return Err(invalid(format!("invalid num_pos: {num_pos}")));
        }
        let defaults = sec.start + 4..sec.start + 4 + num_pos * 2;
        if defaults.end > sec.end {
            return Err(invalid("matrix defaults truncated"));
        }
        let (exc_ref, used) = BitRef::parse(&data[defaults.end..sec.end])?;
        let exceptions = exc_ref.offset(defaults.end);
        let exc = BitView::from_ref(data, &exceptions);
        if exc.len() != num_pos * num_pos {
            return Err(invalid("matrix exception bits size mismatch"));
        }
        let costs = defaults.end + used..sec.end;
        if costs.len() != exc.count_ones() * 2 {
            return Err(invalid("matrix exception cost count mismatch"));
        }
        Ok(MatrixLayout {
            num_pos,
            defaults,
            exceptions,
            costs,
        })
    }
}

/// 固定長レコードのセクションのレコード数 (長さがレコード長の倍数でなければエラー)
fn section_count(
    sections: &[Range<usize>; SECTION_COUNT],
    idx: usize,
    record_len: usize,
    name: &str,
) -> io::Result<usize> {
    let len = sections[idx].len();
    if !len.is_multiple_of(record_len) {
        return Err(invalid(format!("{name} section misaligned")));
    }
    Ok(len / record_len)
}

/// セクションを 8 バイト境界に揃えて書き出すライター
pub struct SectionWriter {
    buf: Vec<u8>,
    toc: Vec<(u32, u32)>,
}

impl Default for SectionWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl SectionWriter {
    pub fn new() -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&VERSION.to_le_bytes());
        buf.extend_from_slice(&(SECTION_COUNT as u32).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.resize(HEADER_LEN, 0);
        Self {
            buf,
            toc: Vec::new(),
        }
    }

    /// セクションを 1 本追加する (呼び出し順 = セクション番号)
    pub fn section(&mut self, body: &[u8]) {
        while !self.buf.len().is_multiple_of(8) {
            self.buf.push(0);
        }
        self.toc.push((self.buf.len() as u32, body.len() as u32));
        self.buf.extend_from_slice(body);
    }

    /// TOC を書き込んで完成したバイト列を返す
    pub fn finish(mut self) -> io::Result<Vec<u8>> {
        if self.toc.len() != SECTION_COUNT {
            return Err(invalid(format!(
                "expected {SECTION_COUNT} sections, got {}",
                self.toc.len()
            )));
        }
        for (i, &(off, len)) in self.toc.iter().enumerate() {
            self.buf[16 + i * 8..20 + i * 8].copy_from_slice(&off.to_le_bytes());
            self.buf[20 + i * 8..24 + i * 8].copy_from_slice(&len.to_le_bytes());
        }
        Ok(self.buf)
    }
}
