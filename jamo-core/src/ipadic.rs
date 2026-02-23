use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::bitvec::BitVec;
use crate::dict::{Dictionary, Entry};
use crate::louds::LoudsTrie;
use crate::viterbi::ConnectionCost;

/// IPAdic CSV からパースしたエントリ
#[derive(Debug)]
struct RawEntry {
    surface: String,
    left_id: u16,
    right_id: u16,
    cost: i16,
    reading: String,
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

        // 全 CSV ファイルをパース
        for entry in fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))? {
            let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "csv") {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("read {}: {e}", path.display()))?;
                parse_csv(&content, &mut raw_entries)?;
            }
        }

        // 読みごとにグループ化
        let mut grouped: HashMap<String, Vec<Entry>> = HashMap::new();
        for raw in &raw_entries {
            let reading = katakana_to_hiragana(&raw.reading);
            grouped.entry(reading).or_default().push(Entry {
                surface: raw.surface.clone(),
                left_id: raw.left_id,
                right_id: raw.right_id,
                cost: raw.cost,
            });
        }

        let vocab_size = grouped.values().map(|v| v.len()).sum();
        let reading_count = grouped.len();

        // ソート済みキーリストを構築して LOUDS trie を作る
        let mut sorted: Vec<(String, Vec<Entry>)> = grouped.into_iter().collect();
        sorted.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));

        let keys: Vec<(&[u8], &[Entry])> = sorted
            .iter()
            .map(|(k, v)| (k.as_bytes(), v.as_slice()))
            .collect();
        let trie = LoudsTrie::build(&keys);

        // matrix.def をパース
        let matrix_path = dir.join("matrix.def");
        let matrix_content =
            fs::read_to_string(&matrix_path).map_err(|e| format!("read matrix.def: {e}"))?;
        let (num_pos, matrix) = parse_matrix(&matrix_content)?;

        Ok(IpaDict {
            trie,
            matrix,
            num_pos,
            vocab_size,
            reading_count,
        })
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
}

const MAGIC: &[u8; 4] = b"JAMO";
const VERSION: u32 = 1;

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
    let mut blocks = vec![0u64; num_blocks];
    for block in &mut blocks {
        let mut buf = [0u8; 8];
        r.read_exact(&mut buf)?;
        *block = u64::from_le_bytes(buf);
    }
    let bits = BitVec::from_raw(blocks, bits_len);

    // Labels
    let labels_count = read_u32(r)? as usize;
    let mut labels = vec![0u8; labels_count];
    r.read_exact(&mut labels)?;

    // Terminal
    let terminal_count = read_u32(r)? as usize;
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
        entries.push(Entry {
            surface,
            left_id,
            right_id,
            cost,
        });
    }

    let trie = LoudsTrie::from_parts(bits, labels, terminal, entries);

    // Matrix
    let num_pos = read_u32(r)? as usize;
    let matrix_size = num_pos * num_pos;
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

impl ConnectionCost for IpaDict {
    fn cost(&self, right_id: u16, left_id: u16) -> i32 {
        self.connection_cost(right_id, left_id) as i32
    }
}

impl Dictionary for IpaDict {
    fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)> {
        self.trie.common_prefix_search(input)
    }
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

/// IPAdic CSV の1行をパース
fn parse_csv_line(line: &str) -> Option<RawEntry> {
    // フォーマット: 表層形,left_id,right_id,cost,品詞,...,原形,読み,発音
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 12 {
        return None;
    }

    let surface = fields[0].to_string();
    let left_id: u16 = fields[1].parse().ok()?;
    let right_id: u16 = fields[2].parse().ok()?;
    let cost: i16 = fields[3].parse().ok()?;
    let reading = fields[11].to_string(); // 読み (カタカナ)

    Some(RawEntry {
        surface,
        left_id,
        right_id,
        cost,
        reading,
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
mod tests {
    use super::*;

    #[test]
    fn test_katakana_to_hiragana() {
        assert_eq!(katakana_to_hiragana("キョウ"), "きょう");
        assert_eq!(katakana_to_hiragana("テンキ"), "てんき");
        assert_eq!(katakana_to_hiragana("ハ"), "は");
    }

    #[test]
    fn test_parse_csv_line() {
        let line = "今日,1285,1285,5543,名詞,一般,*,*,*,*,今日,キョウ,キョー";
        let entry = parse_csv_line(line).unwrap();
        assert_eq!(entry.surface, "今日");
        assert_eq!(entry.left_id, 1285);
        assert_eq!(entry.right_id, 1285);
        assert_eq!(entry.cost, 5543);
        assert_eq!(entry.reading, "キョウ");
    }

    #[test]
    fn test_parse_matrix() {
        let content = "3 3\n0 0 -100\n0 1 200\n1 0 -50\n2 2 300\n";
        let (n, matrix) = parse_matrix(content).unwrap();
        assert_eq!(n, 3);
        assert_eq!(matrix[0 * 3 + 0], -100);
        assert_eq!(matrix[0 * 3 + 1], 200);
        assert_eq!(matrix[1 * 3 + 0], -50);
        assert_eq!(matrix[2 * 3 + 2], 300);
    }

    #[test]
    fn test_convert_with_ipadic() {
        let ipadic_dir = Path::new("../data/ipadic");
        if !ipadic_dir.exists() {
            eprintln!("Skipping: IPAdic not found");
            return;
        }

        let dict = IpaDict::load(ipadic_dir).expect("failed to load IPAdic");
        let result = crate::convert_with_conn("きょうはいいてんきですね", &dict, &dict);

        let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
        eprintln!("変換結果: {text}");

        // 最低限、入力全体をカバーしているはず
        let yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
        assert_eq!(yomi, "きょうはいいてんきですね");

        // 接続コスト付きで「天気」が出るようになった (Iter 1 では「天木」だった)
        assert!(text.contains("天気"), "「天気」が含まれるはず: {text}");
    }

    #[test]
    fn test_load_ipadic() {
        let ipadic_dir = Path::new("../data/ipadic");
        if !ipadic_dir.exists() {
            eprintln!("Skipping: IPAdic not found at {}", ipadic_dir.display());
            return;
        }

        let dict = IpaDict::load(ipadic_dir).expect("failed to load IPAdic");
        eprintln!(
            "Loaded {} vocab, {} readings, {} pos",
            dict.vocab_size(),
            dict.reading_count(),
            dict.pos_size()
        );

        assert!(
            dict.vocab_size() > 300_000,
            "IPAdic should have >300k entries"
        );
        assert!(dict.pos_size() > 1000, "IPAdic should have >1000 POS IDs");

        // 「きょう」で引けるはず
        let results = dict.common_prefix_search("きょう");
        assert!(!results.is_empty(), "「きょう」がヒットするはず");

        let surfaces: Vec<&str> = results
            .iter()
            .flat_map(|(_, entries)| entries.iter().map(|e| e.surface.as_str()))
            .collect();
        assert!(
            surfaces.contains(&"今日"),
            "「今日」が候補にあるはず: {:?}",
            surfaces
        );
    }

    #[test]
    fn test_binary_roundtrip() {
        let ipadic_dir = Path::new("../data/ipadic");
        if !ipadic_dir.exists() {
            eprintln!("Skipping: IPAdic not found");
            return;
        }

        let dict = IpaDict::load(ipadic_dir).expect("failed to load IPAdic");

        // バイナリに書き出し
        let tmp = std::env::temp_dir().join("jamo_test_dict.bin");
        dict.save_binary(&tmp).expect("failed to save binary");

        let file_size = fs::metadata(&tmp).unwrap().len();
        eprintln!(
            "Binary dict size: {} bytes ({:.1} MB)",
            file_size,
            file_size as f64 / 1_048_576.0
        );

        // バイナリから読み込み
        let dict2 = IpaDict::load_binary(&tmp).expect("failed to load binary");

        // 統計が一致
        assert_eq!(dict.vocab_size(), dict2.vocab_size());
        assert_eq!(dict.reading_count(), dict2.reading_count());
        assert_eq!(dict.pos_size(), dict2.pos_size());

        // 変換結果が一致
        let result1 = crate::convert_with_conn("わたしはがくせいです", &dict, &dict);
        let result2 = crate::convert_with_conn("わたしはがくせいです", &dict2, &dict2);
        let text1: String = result1
            .segments
            .iter()
            .map(|s| s.surface.as_str())
            .collect();
        let text2: String = result2
            .segments
            .iter()
            .map(|s| s.surface.as_str())
            .collect();
        assert_eq!(text1, text2);

        // 接続コストが一致
        assert_eq!(dict.connection_cost(0, 0), dict2.connection_cost(0, 0));
        assert_eq!(
            dict.connection_cost(100, 200),
            dict2.connection_cost(100, 200)
        );

        // クリーンアップ
        let _ = fs::remove_file(&tmp);
    }
}
