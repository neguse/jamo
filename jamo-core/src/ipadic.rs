use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::bindict::{self, BinDict};
use crate::dict::Entry;

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

/// 辞書ソース (IPAdic CSV / mozc TSV) をパースした、ビルド専用の中間辞書。
/// 変換には使えない。`build()` で v3 バイナリにして `BinDict` で開く
pub struct IpaDict {
    /// ひらがな読み → エントリ一覧 (読みのバイト順)
    keys: Vec<(String, Vec<Entry>)>,
    /// 接続コスト行列 [right_id * num_pos + left_id] = cost
    matrix: Vec<i16>,
    /// 品詞ID数
    num_pos: usize,
    /// 語彙数
    vocab_size: usize,
    /// 品詞 ID ごとの文節境界ペナルティ (mozc boundary.def)。空なら全て 0
    boundary: Vec<(u16, u16)>,
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

        build_from_entries(raw_entries, matrix, num_pos, Vec::new())
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

        build_from_entries(raw_entries, matrix, num_pos, Vec::new())
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
        self.keys.len()
    }

    /// 品詞ID数
    pub fn pos_size(&self) -> usize {
        self.num_pos
    }

    /// v3 バイナリを構築する
    pub fn build(&self) -> io::Result<Vec<u8>> {
        bindict::build(bindict::BuildInput {
            keys: &self.keys,
            matrix: &self.matrix,
            num_pos: self.num_pos,
            boundary: &self.boundary,
        })
    }

    /// v3 バイナリを構築して変換用辞書として開く
    pub fn into_bindict(self) -> io::Result<BinDict> {
        BinDict::from_vec(self.build()?)
    }

    /// バイナリ辞書を書き出す
    pub fn save_binary(&self, path: &Path) -> io::Result<()> {
        fs::write(path, self.build()?)
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
    boundary: Vec<(u16, u16)>,
) -> Result<IpaDict, String> {
    let mut grouped: HashMap<String, Vec<Entry>> = HashMap::new();
    for raw in &raw_entries {
        // 表層か読みが空のエントリは変換に使えない (IPAdic の "," は CSV 分割で表層が空になる)
        if raw.surface.is_empty() || raw.reading.is_empty() {
            continue;
        }
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
    let mut keys: Vec<(String, Vec<Entry>)> = grouped.into_iter().collect();
    keys.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));

    Ok(IpaDict {
        keys,
        matrix,
        num_pos,
        vocab_size,
        boundary,
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
