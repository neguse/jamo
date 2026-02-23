use crate::dict::Dictionary;

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
    /// Viterbi累積コスト (デコード時に設定)
    pub total_cost: i32,
    /// 最適前ノードのインデックス (デコード時に設定)
    pub prev: Option<usize>,
}

/// ラティス: 各バイト位置で終了するノード群を管理
pub struct Lattice {
    /// 全ノード
    pub nodes: Vec<Node>,
    /// ends_at[byte_pos] = そのバイト位置で終了するノードのインデックス一覧
    pub ends_at: Vec<Vec<usize>>,
    /// 入力のバイト長
    pub input_len: usize,
}

impl Lattice {
    /// 辞書を使ってラティスを構築
    pub fn build(input: &str, dict: &dyn Dictionary) -> Self {
        let input_len = input.len();
        let mut nodes = Vec::new();
        let mut ends_at = vec![Vec::new(); input_len + 1];

        // BOS (文頭) ノード: 位置0で終了する特殊ノード
        let bos_idx = nodes.len();
        nodes.push(Node {
            start: 0,
            end: 0,
            surface: String::new(),
            left_id: 0,
            right_id: 0,
            word_cost: 0,
            total_cost: 0,
            prev: None,
        });
        ends_at[0].push(bos_idx);

        // 各文字位置から辞書引き
        for (char_start, _) in input.char_indices() {
            let remaining = &input[char_start..];
            let results = dict.common_prefix_search(remaining);
            for (match_len, entries) in results {
                let byte_end = char_start + match_len;
                for entry in entries {
                    let idx = nodes.len();
                    nodes.push(Node {
                        start: char_start,
                        end: byte_end,
                        surface: entry.surface.clone(),
                        left_id: entry.left_id,
                        right_id: entry.right_id,
                        word_cost: entry.cost as i32,
                        total_cost: i32::MAX,
                        prev: None,
                    });
                    ends_at[byte_end].push(idx);
                }
            }
        }

        // EOS (文末) ノード: 入力末尾で開始・終了する特殊ノード
        let eos_idx = nodes.len();
        nodes.push(Node {
            start: input_len,
            end: input_len,
            surface: String::new(),
            left_id: 0,
            right_id: 0,
            word_cost: 0,
            total_cost: i32::MAX,
            prev: None,
        });
        ends_at[input_len].push(eos_idx);

        Lattice {
            nodes,
            ends_at,
            input_len,
        }
    }

    /// BOS ノードのインデックス (常に 0)
    pub fn bos_index(&self) -> usize {
        0
    }

    /// EOS ノードのインデックス (常に最後)
    pub fn eos_index(&self) -> usize {
        self.nodes.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::HashMapDict;

    #[test]
    fn test_lattice_build_kyou() {
        let dict = HashMapDict::test_dict();
        let lattice = Lattice::build("きょう", &dict);

        // BOS + "き"(木) + "き"(気) + "きょう"(今日) + "きょう"(京) + EOS = 6 ノード
        assert!(
            lattice.nodes.len() >= 4,
            "少なくとも BOS + 辞書エントリ + EOS が必要: got {}",
            lattice.nodes.len()
        );

        // BOS は位置0で終了
        assert!(!lattice.ends_at[0].is_empty());

        // "きょう" は9バイトなので位置9で終了するノードがあるはず
        let at_9: Vec<&str> = lattice.ends_at[9]
            .iter()
            .map(|&i| lattice.nodes[i].surface.as_str())
            .collect();
        assert!(
            at_9.contains(&"今日"),
            "位置9に「今日」があるはず: {:?}",
            at_9
        );
        assert!(at_9.contains(&"京"), "位置9に「京」があるはず: {:?}", at_9);
    }

    #[test]
    fn test_lattice_build_kyouha() {
        let dict = HashMapDict::test_dict();
        let lattice = Lattice::build("きょうは", &dict);

        // "は" は位置9-12 (3bytes)
        let at_12: Vec<&str> = lattice.ends_at[12]
            .iter()
            .map(|&i| lattice.nodes[i].surface.as_str())
            .collect();
        assert!(
            at_12.contains(&"は"),
            "位置12に「は」があるはず: {:?}",
            at_12
        );
    }

    #[test]
    fn test_lattice_has_bos_eos() {
        let dict = HashMapDict::test_dict();
        let lattice = Lattice::build("きょう", &dict);

        let bos = &lattice.nodes[lattice.bos_index()];
        assert_eq!(bos.start, 0);
        assert_eq!(bos.end, 0);

        let eos = &lattice.nodes[lattice.eos_index()];
        assert_eq!(eos.start, 9); // "きょう" = 9 bytes
        assert_eq!(eos.end, 9);
    }
}
