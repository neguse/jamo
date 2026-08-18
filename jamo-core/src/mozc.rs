//! mozc dictionary_oss を辞書ソースとしてロードする
//!
//! 語彙 + 単語コスト + 品詞ID + 接続行列を三位一体で使う。
//! コストだけ・順位だけの部分移植は品詞IDと接続行列の整合が崩れて
//! 精度が大きく悪化することを実測済み (doc/tasks.md Iter 8 参照)。
//!
//! 必要ファイル (mozc リポジトリの src/data/dictionary_oss/):
//! - `dictionary*.txt` : 読み\t左文脈ID\t右文脈ID\tコスト\t表層形
//! - `id.def`          : ID と品詞素性 (IPAdic 互換のカンマ区切り)
//! - `connection_single_column.txt` : 1行目が品詞数、以降 [right][left] 順の接続コスト

use std::fs;
use std::path::Path;

use crate::dict::POS_CONTENT;
use crate::ipadic::{IpaDict, RawEntry, build_from_entries, classify_pos};

/// mozc dictionary_oss ディレクトリから辞書をロードする
pub fn load_mozc(dir: &Path) -> Result<IpaDict, String> {
    let pos_type = parse_id_def(&read(dir, "id.def")?)?;
    let (num_pos, matrix) = parse_connection(&read(dir, "connection_single_column.txt")?)?;

    let mut entries = Vec::new();
    let mut dict_files: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("dictionary") && n.ends_with(".txt"))
        })
        .collect();
    dict_files.sort();
    if dict_files.is_empty() {
        return Err(format!("no dictionary*.txt in {}", dir.display()));
    }
    for path in dict_files {
        let content =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        parse_dictionary(&content, &pos_type, &mut entries);
    }

    build_from_entries(entries, matrix, num_pos)
}

fn read(dir: &Path, name: &str) -> Result<String, String> {
    let path = dir.join(name);
    fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))
}

/// id.def をパースして ID → 品詞タイプ表を返す
fn parse_id_def(content: &str) -> Result<Vec<u8>, String> {
    let mut pos_type = Vec::new();
    for line in content.lines() {
        let mut it = line.splitn(2, ' ');
        let (Some(id), Some(feat)) = (it.next(), it.next()) else {
            continue;
        };
        let id: usize = id.parse().map_err(|e| format!("id.def id: {e}"))?;
        let mut f = feat.split(',');
        let pos = f.next().unwrap_or("*");
        let sub = f.next().unwrap_or("*");
        if id >= pos_type.len() {
            pos_type.resize(id + 1, POS_CONTENT);
        }
        pos_type[id] = classify_pos(pos, sub);
    }
    if pos_type.is_empty() {
        return Err("empty id.def".to_string());
    }
    Ok(pos_type)
}

/// connection_single_column.txt をパース
fn parse_connection(content: &str) -> Result<(usize, Vec<i16>), String> {
    let mut lines = content.lines();
    let num_pos: usize = lines
        .next()
        .ok_or("empty connection file")?
        .trim()
        .parse()
        .map_err(|e| format!("connection header: {e}"))?;
    let mut matrix = Vec::with_capacity(num_pos * num_pos);
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: i32 = line.parse().map_err(|e| format!("connection value: {e}"))?;
        matrix.push(v.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    }
    if matrix.len() != num_pos * num_pos {
        return Err(format!(
            "connection size mismatch: expected {}, got {}",
            num_pos * num_pos,
            matrix.len()
        ));
    }
    Ok((num_pos, matrix))
}

/// dictionary*.txt (読み\t左ID\t右ID\tコスト\t表層形) をパース
fn parse_dictionary(content: &str, pos_type: &[u8], entries: &mut Vec<RawEntry>) {
    for line in content.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        let (Ok(left_id), Ok(right_id), Ok(cost)) = (
            f[1].parse::<u16>(),
            f[2].parse::<u16>(),
            f[3].parse::<i16>(),
        ) else {
            continue;
        };
        entries.push(RawEntry {
            surface: f[4].to_string(),
            left_id,
            right_id,
            cost,
            reading: f[0].to_string(),
            pos_type: pos_type
                .get(left_id as usize)
                .copied()
                .unwrap_or(POS_CONTENT),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::{Dictionary, POS_FUNCTION};

    fn write_fixture(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("id.def"),
            "0 BOS/EOS,*,*,*,*,*,*\n1 名詞,一般,*,*,*,*,*\n2 助詞,係助詞,*,*,*,*,は\n3 動詞,自立,*,*,五段・カ行,基本形,*\n",
        )
        .unwrap();
        // 4x4 行列: 全て 100、BOS(0)→名詞(1) だけ 10
        let mut conn = String::from("4\n");
        for right in 0..4 {
            for left in 0..4 {
                let v = if right == 0 && left == 1 { 10 } else { 100 };
                conn.push_str(&format!("{v}\n"));
            }
        }
        fs::write(dir.join("connection_single_column.txt"), conn).unwrap();
        fs::write(
            dir.join("dictionary00.txt"),
            "きょう\t1\t1\t3000\t今日\nきょう\t1\t1\t4000\t京\nは\t2\t2\t500\tは\n",
        )
        .unwrap();
    }

    #[test]
    fn test_load_mozc_fixture() {
        let dir = std::env::temp_dir().join(format!("jamo-mozc-test-{}", std::process::id()));
        write_fixture(&dir);
        let dict = load_mozc(&dir).unwrap();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(dict.pos_size(), 4);
        assert_eq!(dict.connection_cost(0, 1), 10);
        assert_eq!(dict.connection_cost(1, 1), 100);

        let results = dict.common_prefix_search("きょうは");
        let kyou = results
            .iter()
            .find(|(len, _)| *len == "きょう".len())
            .expect("きょう がヒットするはず");
        assert!(kyou.1.iter().any(|e| e.surface == "今日"));
        // 助詞は付属語
        let ha = results.iter().find(|(len, _)| *len == "き".len()).is_none();
        assert!(ha, "き 単独のエントリはない");
        let result = crate::convert("は", &dict);
        assert_eq!(result.segments[0].pos_type, POS_FUNCTION);
    }

    #[test]
    fn test_load_mozc_missing_dir_fails() {
        let dir = std::env::temp_dir().join("jamo-mozc-test-nonexistent");
        assert!(load_mozc(&dir).is_err());
    }

    /// 実データがあればロードと変換のスモーク (無ければ skip)
    #[test]
    fn test_load_mozc_real_data() {
        let dir = Path::new("../data/mozc");
        if !dir.join("id.def").exists() {
            eprintln!("Skipping: data/mozc not found");
            return;
        }
        let dict = load_mozc(dir).unwrap();
        let result = crate::convert_with_conn("きょうはいいてんきですね", &dict, &dict);
        let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
        assert!(!text.is_empty());
        eprintln!("mozc convert: {text}");
    }
}
