/// ラティスノード
#[derive(Debug, Clone)]
pub struct Node {
    /// かな列上の開始バイト位置
    pub start: usize,
    /// かな列上の終了バイト位置
    pub end: usize,
    /// 表層形
    pub surface: String,
    /// 左文脈品詞ID
    pub left_id: u16,
    /// 右文脈品詞ID
    pub right_id: u16,
    /// 単語コスト
    pub word_cost: i32,
    /// Viterbi累積コスト
    pub total_cost: i32,
    /// 最適前ノードのインデックス (BOS=-1 相当は None)
    pub prev: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_can_be_created() {
        let node = Node {
            start: 0,
            end: 6,
            surface: "今日".to_string(),
            left_id: 0,
            right_id: 0,
            word_cost: -1000,
            total_cost: i32::MAX,
            prev: None,
        };
        assert_eq!(node.surface, "今日");
        assert_eq!(node.start, 0);
        assert_eq!(node.end, 6);
    }
}
