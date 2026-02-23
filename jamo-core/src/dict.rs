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
    /// input のバイト位置 0 から始まる共通接頭辞検索
    fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_can_be_created() {
        let entry = Entry {
            surface: "今日".to_string(),
            left_id: 0,
            right_id: 0,
            cost: -1000,
        };
        assert_eq!(entry.surface, "今日");
    }
}
