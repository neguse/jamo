use std::collections::HashMap;

/// 辞書エントリ
#[derive(Debug, Clone)]
pub struct Entry {
    /// 表層形 (例: "今日")
    pub surface: String,
    /// 左文脈品詞ID
    pub left_id: u16,
    /// 右文脈品詞ID
    pub right_id: u16,
    /// 単語コスト (低いほど出やすい)
    pub cost: i16,
}

/// 辞書の共通インターフェース
pub trait Dictionary {
    /// 入力のプレフィックスにマッチする全エントリを返す
    /// 戻り値: (マッチしたバイト長, エントリ一覧) のリスト
    fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)>;
}

/// ハードコードされたテスト用辞書 (HashMap)
pub struct HashMapDict {
    /// 読み → エントリ一覧
    entries: HashMap<String, Vec<Entry>>,
}

impl Default for HashMapDict {
    fn default() -> Self {
        Self::new()
    }
}

impl HashMapDict {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn add(&mut self, yomi: &str, surface: &str, left_id: u16, right_id: u16, cost: i16) {
        self.entries
            .entry(yomi.to_string())
            .or_default()
            .push(Entry {
                surface: surface.to_string(),
                left_id,
                right_id,
                cost,
            });
    }

    /// テスト用の小さな辞書を生成
    pub fn test_dict() -> Self {
        let mut dict = Self::new();
        // 品詞ID: 0=名詞, 1=助詞, 2=形容詞, 3=動詞, 4=助動詞, 5=接尾辞
        dict.add("きょう", "今日", 0, 0, -3000);
        dict.add("きょう", "京", 0, 0, -2000);
        dict.add("き", "木", 0, 0, -2500);
        dict.add("は", "は", 1, 1, -3000);
        dict.add("いい", "いい", 2, 2, -3000);
        dict.add("いい", "良い", 2, 2, -2500);
        dict.add("てんき", "天気", 0, 0, -3500);
        dict.add("てん", "天", 0, 0, -2000);
        dict.add("き", "気", 0, 0, -2000);
        dict.add("です", "です", 4, 4, -3000);
        dict.add("ね", "ね", 1, 1, -2500);
        dict
    }
}

impl Dictionary for HashMapDict {
    fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)> {
        let mut results = Vec::new();
        // 入力の先頭から、1文字ずつ長くしてマッチを探す
        let mut end = 0;
        for (i, _) in input.char_indices() {
            // i は次の文字の開始バイト位置を得るために使う
            // char_indices は (byte_offset, char) を返す
            // 最初の文字自体はスキップして、各文字の終端で辞書を引く
            if i == 0 {
                continue;
            }
            end = i;
            let prefix = &input[..i];
            if let Some(entries) = self.entries.get(prefix) {
                results.push((i, entries.clone()));
            }
        }
        // 最後の文字を含む全体もチェック
        if !input.is_empty() {
            let full_len = input.len();
            if full_len != end
                && let Some(entries) = self.entries.get(input)
            {
                results.push((full_len, entries.clone()));
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dict_creation() {
        let dict = HashMapDict::test_dict();
        assert!(dict.entries.contains_key("きょう"));
        assert!(dict.entries.contains_key("は"));
    }

    #[test]
    fn test_common_prefix_search_kyou() {
        let dict = HashMapDict::test_dict();
        let results = dict.common_prefix_search("きょう");
        // "き" (3 bytes) と "きょう" (9 bytes) がマッチするはず
        let lengths: Vec<usize> = results.iter().map(|(len, _)| *len).collect();
        assert!(
            lengths.contains(&3),
            "「き」がマッチするはず: {:?}",
            lengths
        );
        assert!(
            lengths.contains(&9),
            "「きょう」がマッチするはず: {:?}",
            lengths
        );
    }

    #[test]
    fn test_common_prefix_search_kyouha() {
        let dict = HashMapDict::test_dict();
        let results = dict.common_prefix_search("きょうは");
        let lengths: Vec<usize> = results.iter().map(|(len, _)| *len).collect();
        // "き" (3 bytes), "きょう" (9 bytes) がマッチ
        // "きょうは" は辞書にないのでマッチしない
        assert!(lengths.contains(&3));
        assert!(lengths.contains(&9));
        assert!(!lengths.contains(&12));
    }

    #[test]
    fn test_common_prefix_search_tenki() {
        let dict = HashMapDict::test_dict();
        let results = dict.common_prefix_search("てんき");
        let lengths: Vec<usize> = results.iter().map(|(len, _)| *len).collect();
        // "てん" (6 bytes), "てんき" (9 bytes) がマッチ
        assert!(
            lengths.contains(&6),
            "「てん」がマッチするはず: {:?}",
            lengths
        );
        assert!(
            lengths.contains(&9),
            "「てんき」がマッチするはず: {:?}",
            lengths
        );
    }

    #[test]
    fn test_multiple_candidates() {
        let dict = HashMapDict::test_dict();
        let results = dict.common_prefix_search("きょう");
        // "きょう" に対して "今日" と "京" の2候補があるはず
        for (len, entries) in &results {
            if *len == 9 {
                assert_eq!(entries.len(), 2);
                let surfaces: Vec<&str> = entries.iter().map(|e| e.surface.as_str()).collect();
                assert!(surfaces.contains(&"今日"));
                assert!(surfaces.contains(&"京"));
            }
        }
    }

    #[test]
    fn test_empty_input() {
        let dict = HashMapDict::test_dict();
        let results = dict.common_prefix_search("");
        assert!(results.is_empty());
    }

    #[test]
    fn test_no_match() {
        let dict = HashMapDict::test_dict();
        let results = dict.common_prefix_search("zzz");
        assert!(results.is_empty());
    }
}
