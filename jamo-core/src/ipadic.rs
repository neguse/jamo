use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::bitvec::BitVec;
use crate::dict::{Dictionary, Entry};
use crate::louds::LoudsTrie;
use crate::viterbi::ConnectionCost;

/// 辞書ソース (IPAdic CSV / mozc TSV) からパースしたエントリ
#[derive(Debug)]
pub(crate) struct RawEntry {
    pub(crate) surface: String,
    pub(crate) left_id: u16,
    pub(crate) right_id: u16,
    pub(crate) cost: i16,
    pub(crate) reading: String,
    pub(crate) pos_type: u8,
}

/// IPAdic から構築した辞書
pub struct IpaDict {
    /// LOUDS trie (読み → エントリ)
    trie: LoudsTrie,
    /// 接続コスト行列 [left_id * num_right + right_id] = cost
    matrix: Vec<i16>,
    /// 品詞ID数
    num_pos: usize,
    /// 語彙数
    vocab_size: usize,
    /// 読みの種類数
    reading_count: usize,
}

impl IpaDict {
    /// IPAdic ディレクトリからロード
    pub fn load(dir: &Path) -> Result<Self, String> {
        let mut raw_entries = Vec::new();
        parse_csv_dir(dir, &mut raw_entries)?;

        let matrix_path = dir.join("matrix.def");
        let matrix_content =
            fs::read_to_string(&matrix_path).map_err(|e| format!("read matrix.def: {e}"))?;
        let (num_pos, matrix) = parse_matrix(&matrix_content)?;

        build_from_entries(raw_entries, matrix, num_pos)
    }

    /// ベース辞書 + 追加CSVをマージしてロード (NEologd等の統合用)
    /// matrix.def はベースディレクトリのものを使用。
    pub fn load_merge(base_dir: &Path, extras: &[&Path]) -> Result<Self, String> {
        let mut raw_entries = Vec::new();
        parse_csv_dir(base_dir, &mut raw_entries)?;

        for extra in extras {
            if extra.is_dir() {
                parse_csv_dir(extra, &mut raw_entries)?;
            } else if extra.extension().is_some_and(|ext| ext == "csv") {
                let content = fs::read_to_string(extra)
                    .map_err(|e| format!("read {}: {e}", extra.display()))?;
                parse_csv(&content, &mut raw_entries)?;
            } else {
                return Err(format!(
                    "unsupported extra path: {} (expected .csv file or directory)",
                    extra.display()
                ));
            }
        }

        let matrix_path = base_dir.join("matrix.def");
        let matrix_content =
            fs::read_to_string(&matrix_path).map_err(|e| format!("read matrix.def: {e}"))?;
        let (num_pos, matrix) = parse_matrix(&matrix_content)?;

        build_from_entries(raw_entries, matrix, num_pos)
    }

    /// 接続コスト (right_id of prev, left_id of next)
    pub fn connection_cost(&self, right_id: u16, left_id: u16) -> i16 {
        let idx = right_id as usize * self.num_pos + left_id as usize;
        self.matrix.get(idx).copied().unwrap_or(0)
    }

    /// 語彙数
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    /// 読みの種類数
    pub fn reading_count(&self) -> usize {
        self.reading_count
    }

    /// 品詞ID数
    pub fn pos_size(&self) -> usize {
        self.num_pos
    }

    /// バイナリ辞書を書き出す
    pub fn save_binary(&self, path: &Path) -> io::Result<()> {
        let mut f = io::BufWriter::new(fs::File::create(path)?);
        write_binary(
            &mut f,
            &self.trie,
            &self.matrix,
            self.num_pos,
            self.vocab_size,
            self.reading_count,
        )
    }

    /// バイナリ辞書から読み込む
    pub fn load_binary(path: &Path) -> io::Result<Self> {
        let mut f = io::BufReader::new(fs::File::open(path)?);
        read_binary(&mut f)
    }

    /// バイナリ辞書を Reader から読み込む (WASM 用)
    pub fn load_binary_from_reader(reader: &mut impl Read) -> io::Result<Self> {
        read_binary(reader)
    }
}

const MAGIC: &[u8; 4] = b"JAMO";
const VERSION: u32 = 2;
const MAX_BITS: usize = 100_000_000;
const MAX_LOUDS_NODES: usize = 10_000_000;
const MAX_ENTRIES: usize = 5_000_000;
const MAX_POS: usize = 8192;

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn write_u32(w: &mut impl Write, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_u16(w: &mut impl Write, v: u16) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_i16(w: &mut impl Write, v: i16) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u16(r: &mut impl Read) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i16(r: &mut impl Read) -> io::Result<i16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

fn write_binary(
    w: &mut impl Write,
    trie: &LoudsTrie,
    matrix: &[i16],
    num_pos: usize,
    vocab_size: usize,
    reading_count: usize,
) -> io::Result<()> {
    // Header
    w.write_all(MAGIC)?;
    write_u32(w, VERSION)?;

    let bits = trie.bits();
    let labels = trie.labels();
    let terminal = trie.terminal();
    let entries = trie.entries();

    // BitVec: len + blocks
    write_u32(w, bits.len() as u32)?;
    let blocks = bits.blocks();
    write_u32(w, blocks.len() as u32)?;
    for &block in blocks {
        w.write_all(&block.to_le_bytes())?;
    }

    // Labels
    write_u32(w, labels.len() as u32)?;
    w.write_all(labels)?;

    // Terminal: pairs of (start, end) with u32::MAX sentinel for None
    write_u32(w, terminal.len() as u32)?;
    for t in terminal {
        match t {
            Some((start, end)) => {
                write_u32(w, *start)?;
                write_u32(w, *end)?;
            }
            None => {
                write_u32(w, u32::MAX)?;
                write_u32(w, 0)?;
            }
        }
    }

    // Entries
    write_u32(w, entries.len() as u32)?;
    for e in entries {
        let surface_bytes = e.surface.as_bytes();
        write_u16(w, surface_bytes.len() as u16)?;
        w.write_all(surface_bytes)?;
        write_u16(w, e.left_id)?;
        write_u16(w, e.right_id)?;
        write_i16(w, e.cost)?;
        w.write_all(&[e.pos_type])?;
    }

    // Matrix
    write_u32(w, num_pos as u32)?;
    for &cost in matrix {
        write_i16(w, cost)?;
    }

    // Stats
    write_u32(w, vocab_size as u32)?;
    write_u32(w, reading_count as u32)?;

    Ok(())
}

fn read_binary(r: &mut impl Read) -> io::Result<IpaDict> {
    // Header
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }
    let version = read_u32(r)?;
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version: {version}"),
        ));
    }

    // BitVec
    let bits_len = read_u32(r)? as usize;
    let num_blocks = read_u32(r)? as usize;
    if bits_len > MAX_BITS {
        return Err(invalid_data(format!("bits too large: {bits_len}")));
    }
    let expected_blocks = bits_len.div_ceil(64);
    if num_blocks != expected_blocks {
        return Err(invalid_data(format!(
            "invalid bit block count: expected {expected_blocks}, got {num_blocks}"
        )));
    }
    let mut blocks = vec![0u64; num_blocks];
    for block in &mut blocks {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf)?;
        *block = u64::from_le_bytes(buf);
    }
    let bits = BitVec::from_raw(blocks, bits_len);

    // Labels
    let labels_count = read_u32(r)? as usize;
    if labels_count == 0 || labels_count > MAX_LOUDS_NODES {
        return Err(invalid_data(format!(
            "invalid labels count: {labels_count}"
        )));
    }
    let mut labels = vec![0u8; labels_count];
    r.read_exact(&mut labels)?;

    // Terminal
    let terminal_count = read_u32(r)? as usize;
    if terminal_count != labels_count {
        return Err(invalid_data(format!(
            "terminal count must match labels count: labels={labels_count}, terminal={terminal_count}"
        )));
    }
    let mut terminal = Vec::with_capacity(terminal_count);
    for _ in 0..terminal_count {
        let start = read_u32(r)?;
        let end = read_u32(r)?;
        if start == u32::MAX {
            terminal.push(None);
        } else {
            terminal.push(Some((start, end)));
        }
    }

    // Entries
    let entries_count = read_u32(r)? as usize;
    if entries_count > MAX_ENTRIES {
        return Err(invalid_data(format!("entries too large: {entries_count}")));
    }
    let mut entries = Vec::with_capacity(entries_count);
    for _ in 0..entries_count {
        let surface_len = read_u16(r)? as usize;
        let mut surface_bytes = vec![0u8; surface_len];
        r.read_exact(&mut surface_bytes)?;
        let surface = String::from_utf8(surface_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let left_id = read_u16(r)?;
        let right_id = read_u16(r)?;
        let cost = read_i16(r)?;
        let mut pos_buf = [0u8; 1];
        r.read_exact(&mut pos_buf)?;
        let pos_type = pos_buf[0];
        entries.push(Entry {
            surface,
            left_id,
            right_id,
            cost,
            pos_type,
        });
    }

    validate_louds_parts(&bits, &labels, &terminal, entries.len())?;
    let trie = LoudsTrie::from_parts(bits, labels, terminal, entries);

    // Matrix
    let num_pos = read_u32(r)? as usize;
    if num_pos == 0 || num_pos > MAX_POS {
        return Err(invalid_data(format!("invalid num_pos: {num_pos}")));
    }
    let matrix_size = num_pos
        .checked_mul(num_pos)
        .ok_or_else(|| invalid_data(format!("matrix size overflow: {num_pos}")))?;
    let mut matrix = vec![0i16; matrix_size];
    for cost in &mut matrix {
        *cost = read_i16(r)?;
    }

    // Stats
    let vocab_size = read_u32(r)? as usize;
    let reading_count = read_u32(r)? as usize;

    Ok(IpaDict {
        trie,
        matrix,
        num_pos,
        vocab_size,
        reading_count,
    })
}

fn validate_louds_parts(
    bits: &BitVec,
    labels: &[u8],
    terminal: &[Option<(u32, u32)>],
    entries_len: usize,
) -> io::Result<()> {
    if labels.len() != terminal.len() {
        return Err(invalid_data("labels/terminal length mismatch"));
    }
    if labels.len() < 2 {
        return Err(invalid_data("LOUDS must contain super-root and root"));
    }

    let ones = bits.rank1(bits.len());
    let zeros = bits.rank0(bits.len());
    if ones + 1 != labels.len() {
        return Err(invalid_data(format!(
            "LOUDS one-bit/node mismatch: ones={ones}, labels={}",
            labels.len()
        )));
    }
    if zeros != labels.len() {
        return Err(invalid_data(format!(
            "LOUDS zero-bit/node mismatch: zeros={zeros}, labels={}",
            labels.len()
        )));
    }

    for (i, range) in terminal.iter().enumerate() {
        if let Some((start, end)) = range {
            let start = *start as usize;
            let end = *end as usize;
            if start > end || end > entries_len {
                return Err(invalid_data(format!(
                    "terminal range out of entries at node {i}: {start}..{end}, entries={entries_len}"
                )));
            }
        }
    }

    Ok(())
}

impl ConnectionCost for IpaDict {
    fn cost(&self, right_id: u16, left_id: u16) -> i32 {
        self.connection_cost(right_id, left_id) as i32
    }
}

impl IpaDict {
    /// プレフィクス検索: 指定読みで始まるエントリを返す (予測変換用)
    pub fn prefix_search(&self, prefix: &str, limit: usize) -> Vec<Entry> {
        self.trie.prefix_search(prefix, limit)
    }
}

impl Dictionary for IpaDict {
    fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)> {
        self.trie.common_prefix_search(input)
    }
}

/// ディレクトリ内の全CSVファイルをパース
fn parse_csv_dir(dir: &Path, entries: &mut Vec<RawEntry>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "csv") {
            let content =
                fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            parse_csv(&content, entries)?;
        }
    }
    Ok(())
}

/// RawEntry群からIPaDictを構築
pub(crate) fn build_from_entries(
    raw_entries: Vec<RawEntry>,
    matrix: Vec<i16>,
    num_pos: usize,
) -> Result<IpaDict, String> {
    let mut grouped: HashMap<String, Vec<Entry>> = HashMap::new();
    for raw in &raw_entries {
        let reading = katakana_to_hiragana(&raw.reading);
        grouped.entry(reading).or_default().push(Entry {
            surface: raw.surface.clone(),
            left_id: raw.left_id,
            right_id: raw.right_id,
            cost: raw.cost,
            pos_type: raw.pos_type,
        });
    }

    let vocab_size = grouped.values().map(|v| v.len()).sum();
    let reading_count = grouped.len();

    let mut sorted: Vec<(String, Vec<Entry>)> = grouped.into_iter().collect();
    sorted.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));

    let keys: Vec<(&[u8], &[Entry])> = sorted
        .iter()
        .map(|(k, v)| (k.as_bytes(), v.as_slice()))
        .collect();
    let trie = LoudsTrie::build(&keys);

    Ok(IpaDict {
        trie,
        matrix,
        num_pos,
        vocab_size,
        reading_count,
    })
}

/// カタカナをひらがなに変換
fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            // カタカナ (U+30A1..U+30F6) → ひらがな (U+3041..U+3096)
            if (0x30A1..=0x30F6).contains(&cp) {
                char::from_u32(cp - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// 品詞フィールド (IPAdic / mozc id.def 共通の素性形式) から品詞タイプを判定
pub(crate) fn classify_pos(pos: &str, pos_sub: &str) -> u8 {
    use crate::dict::{POS_CONTENT, POS_FUNCTION};
    match pos {
        "助詞" | "助動詞" => POS_FUNCTION,
        "名詞" if pos_sub == "接尾" || pos_sub == "非自立" => POS_FUNCTION,
        "動詞" if pos_sub != "自立" => POS_FUNCTION,
        "形容詞" if pos_sub != "自立" => POS_FUNCTION,
        _ => POS_CONTENT,
    }
}

/// IPAdic CSV の1行をパース
fn parse_csv_line(line: &str) -> Option<RawEntry> {
    // フォーマット: 表層形,left_id,right_id,cost,品詞,品詞細分類1,...,原形,読み,発音
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 12 {
        return None;
    }

    let surface = fields[0].to_string();
    let left_id: u16 = fields[1].parse().ok()?;
    let right_id: u16 = fields[2].parse().ok()?;
    let cost: i16 = fields[3].parse().ok()?;
    let reading = fields[11].to_string(); // 読み (カタカナ)
    let pos_type = classify_pos(fields[4], fields[5]);

    Some(RawEntry {
        surface,
        left_id,
        right_id,
        cost,
        reading,
        pos_type,
    })
}

fn parse_csv(content: &str, entries: &mut Vec<RawEntry>) -> Result<(), String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(entry) = parse_csv_line(line) {
            entries.push(entry);
        }
    }
    Ok(())
}

/// matrix.def をパース
fn parse_matrix(content: &str) -> Result<(usize, Vec<i16>), String> {
    let mut lines = content.lines();

    // 1行目: "1316 1316"
    let header = lines.next().ok_or("empty matrix.def")?;
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() != 2 {
        return Err(format!("invalid matrix header: {header}"));
    }
    let num_left: usize = parts[0].parse().map_err(|e| format!("parse left: {e}"))?;
    let num_right: usize = parts[1].parse().map_err(|e| format!("parse right: {e}"))?;

    if num_left != num_right {
        return Err(format!("asymmetric matrix: {num_left} x {num_right}"));
    }
    let num_pos = num_left;
    let mut matrix = vec![0i16; num_pos * num_pos];

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 3 {
            continue;
        }
        let right_id: usize = parts[0].parse().map_err(|e| format!("parse: {e}"))?;
        let left_id: usize = parts[1].parse().map_err(|e| format!("parse: {e}"))?;
        let cost: i16 = parts[2].parse().map_err(|e| format!("parse: {e}"))?;
        if right_id < num_pos && left_id < num_pos {
            matrix[right_id * num_pos + left_id] = cost;
        }
    }

    Ok((num_pos, matrix))
}

#[cfg(test)]
mod tests;
