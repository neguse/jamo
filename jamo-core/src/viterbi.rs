use crate::lattice::Lattice;

/// 接続コスト: 前ノードの right_id と 次ノードの left_id から接続コストを返す
pub trait ConnectionCost {
    fn cost(&self, right_id: u16, left_id: u16) -> i32;
}

/// 仮の接続コスト (全ペア固定値)
pub struct SimpleConnectionCost;

impl ConnectionCost for SimpleConnectionCost {
    fn cost(&self, _right_id: u16, _left_id: u16) -> i32 {
        // Iter 1 では品詞接続をまだ考慮しない。固定ペナルティ。
        0
    }
}

/// 変換結果の1文節
#[derive(Debug, Clone)]
pub struct Segment {
    /// 表層形
    pub surface: String,
    /// 読み (入力のかな列の該当部分)
    pub yomi: String,
}

/// 変換結果
#[derive(Debug, Clone)]
pub struct ConvertResult {
    pub segments: Vec<Segment>,
}

/// Viterbi デコードを実行して最適パスを返す
pub fn decode(lattice: &mut Lattice, input: &str, conn: &dyn ConnectionCost) -> ConvertResult {
    let input_len = lattice.input_len;

    // 前進: 各ノードの最適コストと最適前ノードを計算
    // バイト位置を左から右へ走査
    for pos in 0..=input_len {
        // この位置で終了するノードのインデックスを取得
        let node_indices: Vec<usize> = lattice.ends_at[pos].clone();
        for &node_idx in &node_indices {
            if node_idx == lattice.bos_index() {
                continue; // BOS は初期化済み
            }

            let node_start = lattice.nodes[node_idx].start;
            let node_left_id = lattice.nodes[node_idx].left_id;
            let node_word_cost = lattice.nodes[node_idx].word_cost;

            // このノードの開始位置で終了する全ノードが前ノード候補
            let prev_indices: Vec<usize> = lattice.ends_at[node_start].clone();
            let mut best_cost = i32::MAX;
            let mut best_prev = None;

            for &prev_idx in &prev_indices {
                let prev_total = lattice.nodes[prev_idx].total_cost;
                if prev_total == i32::MAX {
                    continue; // まだ到達不能
                }
                let prev_right_id = lattice.nodes[prev_idx].right_id;
                let c = conn.cost(prev_right_id, node_left_id);
                let total = prev_total + c + node_word_cost;
                if total < best_cost {
                    best_cost = total;
                    best_prev = Some(prev_idx);
                }
            }

            if best_cost < lattice.nodes[node_idx].total_cost {
                lattice.nodes[node_idx].total_cost = best_cost;
                lattice.nodes[node_idx].prev = best_prev;
            }
        }
    }

    // 後退: EOS から BOS へ最適パスをたどる
    let mut path = Vec::new();
    let mut current = lattice.eos_index();
    while let Some(prev) = lattice.nodes[current].prev {
        if prev == lattice.bos_index() {
            break;
        }
        path.push(prev);
        current = prev;
    }
    path.reverse();

    // パスからセグメントを生成
    let segments = path
        .iter()
        .map(|&idx| {
            let node = &lattice.nodes[idx];
            Segment {
                surface: node.surface.clone(),
                yomi: input[node.start..node.end].to_string(),
            }
        })
        .collect();

    ConvertResult { segments }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::HashMapDict;

    #[test]
    fn test_viterbi_kyou() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょう", &dict);
        let conn = SimpleConnectionCost;
        let result = decode(&mut lattice, "きょう", &conn);

        assert_eq!(result.segments.len(), 1);
        assert_eq!(result.segments[0].surface, "今日");
        assert_eq!(result.segments[0].yomi, "きょう");
    }

    #[test]
    fn test_viterbi_kyouha() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょうは", &dict);
        let conn = SimpleConnectionCost;
        let result = decode(&mut lattice, "きょうは", &conn);

        // 「今日」+「は」の2文節
        assert_eq!(
            result.segments.len(),
            2,
            "2文節のはず: {:?}",
            result.segments
        );
        assert_eq!(result.segments[0].surface, "今日");
        assert_eq!(result.segments[1].surface, "は");
    }

    #[test]
    fn test_viterbi_tenki() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("てんき", &dict);
        let conn = SimpleConnectionCost;
        let result = decode(&mut lattice, "てんき", &conn);

        // 接続コストなしでは単語コストの合計で決まる:
        //   「天気」= -3500
        //   「天」(-2000) + 「木」(-2500) = -4500  ← これが最小
        // 品詞接続コストが入れば「天気」が勝つようになる (Iter 2 で対応)
        let combined_yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
        assert_eq!(combined_yomi, "てんき", "入力全体をカバーするはず");
    }

    #[test]
    fn test_viterbi_result_covers_input() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょうは", &dict);
        let conn = SimpleConnectionCost;
        let result = decode(&mut lattice, "きょうは", &conn);

        // 全セグメントの読みを連結すると入力と一致するはず
        let combined_yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
        assert_eq!(combined_yomi, "きょうは");
    }
}
