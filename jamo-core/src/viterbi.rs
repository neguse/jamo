use crate::dict::POS_CONTENT;
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

/// 変換結果の1単語
#[derive(Debug, Clone)]
pub struct Segment {
    /// 表層形 (最良候補)
    pub surface: String,
    /// 読み (入力のかな列の該当部分)
    pub yomi: String,
    /// 変換候補 (コスト順、最良が先頭)
    pub candidates: Vec<String>,
    /// 品詞タイプ (POS_CONTENT or POS_FUNCTION)
    pub pos_type: u8,
}

/// 文節 (自立語 + 付属語のグループ)
#[derive(Debug, Clone)]
pub struct Clause {
    /// 表層形 (文節全体)
    pub surface: String,
    /// 読み (文節全体)
    pub yomi: String,
    /// 変換候補 (文節単位)
    pub candidates: Vec<String>,
}

/// 変換結果
#[derive(Debug, Clone)]
pub struct ConvertResult {
    /// 単語単位のセグメント
    pub segments: Vec<Segment>,
    /// 文節単位のグループ
    pub clauses: Vec<Clause>,
}

/// Viterbi デコードを実行して最適パスを返す
pub fn decode(lattice: &mut Lattice, input: &str, conn: &dyn ConnectionCost) -> ConvertResult {
    decode_nbest(lattice, input, conn, 1)
}

/// Viterbi デコードを実行して N-best 候補付きの結果を返す
pub fn decode_nbest(
    lattice: &mut Lattice,
    input: &str,
    conn: &dyn ConnectionCost,
    n: usize,
) -> ConvertResult {
    let input_len = lattice.input_len;

    // 前進: 各ノードの最適コストと最適前ノードを計算
    for pos in 0..=input_len {
        let node_indices: Vec<usize> = lattice.ends_at[pos].clone();
        for &node_idx in &node_indices {
            if node_idx == lattice.bos_index() {
                continue;
            }

            let node_start = lattice.nodes[node_idx].start;
            let node_left_id = lattice.nodes[node_idx].left_id;
            let node_word_cost = lattice.nodes[node_idx].word_cost;

            let prev_indices: Vec<usize> = lattice.ends_at[node_start].clone();
            let mut best_cost = i32::MAX;
            let mut best_prev = None;

            for &prev_idx in &prev_indices {
                let prev_total = lattice.nodes[prev_idx].total_cost;
                if prev_total == i32::MAX {
                    continue;
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

    // パスからセグメントを生成 (N-best 候補付き)
    let n = n.max(1);
    let segments: Vec<Segment> = path
        .iter()
        .map(|&idx| {
            let node = &lattice.nodes[idx];
            let surface = node.surface.clone();
            let yomi = input[node.start..node.end].to_string();

            // 同じ (start, end) 区間の他の候補を集める
            let mut others: Vec<(i32, String)> = lattice.ends_at[node.end]
                .iter()
                .filter_map(|&other_idx| {
                    let other = &lattice.nodes[other_idx];
                    if other.start == node.start
                        && other.end == node.end
                        && !other.surface.is_empty()
                        && other.surface != surface
                    {
                        Some((other.word_cost, other.surface.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            others.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            others.dedup_by(|a, b| a.1 == b.1);

            // Viterbi 最適を先頭に、残りを追加
            let mut candidates = vec![surface.clone()];
            candidates.extend(others.into_iter().take(n - 1).map(|(_, s)| s));

            Segment {
                surface,
                yomi,
                candidates,
                pos_type: node.pos_type,
            }
        })
        .collect();

    let clauses = group_clauses(&segments);
    ConvertResult { segments, clauses }
}

/// 単語セグメントを文節にグルーピングする
/// 自立語 (POS_CONTENT) が来たら新しい文節を開始
/// 付属語 (POS_FUNCTION) は直前の文節に付く
pub fn group_clauses(segments: &[Segment]) -> Vec<Clause> {
    if segments.is_empty() {
        return Vec::new();
    }

    // 文節境界を決定: 各セグメントがどの文節に属するか
    let mut clause_groups: Vec<Vec<usize>> = Vec::new();

    for (i, seg) in segments.iter().enumerate() {
        if seg.pos_type == POS_CONTENT || clause_groups.is_empty() {
            // 自立語 or 最初のセグメント → 新文節開始
            clause_groups.push(vec![i]);
        } else {
            // 付属語 → 現在の文節に追加
            clause_groups.last_mut().unwrap().push(i);
        }
    }

    // 各文節の候補を生成
    clause_groups
        .iter()
        .map(|indices| {
            let surface: String = indices
                .iter()
                .map(|&i| segments[i].surface.as_str())
                .collect();
            let yomi: String = indices.iter().map(|&i| segments[i].yomi.as_str()).collect();

            // 候補生成: ベスト + 各単語の代替候補を使ったバリエーション
            let mut candidates = vec![surface.clone()];
            for (pos, &seg_idx) in indices.iter().enumerate() {
                let seg = &segments[seg_idx];
                for alt in seg.candidates.iter().skip(1) {
                    let variant: String = indices
                        .iter()
                        .enumerate()
                        .map(|(j, &idx)| {
                            if j == pos {
                                alt.as_str()
                            } else {
                                segments[idx].surface.as_str()
                            }
                        })
                        .collect();
                    if !candidates.contains(&variant) {
                        candidates.push(variant);
                    }
                }
            }

            Clause {
                surface,
                yomi,
                candidates,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::{HashMapDict, POS_CONTENT, POS_FUNCTION};

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

        let combined_yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
        assert_eq!(combined_yomi, "てんき", "入力全体をカバーするはず");
    }

    #[test]
    fn test_viterbi_result_covers_input() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょうは", &dict);
        let conn = SimpleConnectionCost;
        let result = decode(&mut lattice, "きょうは", &conn);

        let combined_yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
        assert_eq!(combined_yomi, "きょうは");
    }

    #[test]
    fn test_nbest_candidates() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょう", &dict);
        let conn = SimpleConnectionCost;
        let result = decode_nbest(&mut lattice, "きょう", &conn, 5);

        assert_eq!(result.segments.len(), 1);
        let candidates = &result.segments[0].candidates;

        // 「きょう」には「今日」と「京」の2候補があるはず
        assert!(candidates.len() >= 2, "2候補以上あるはず: {candidates:?}");
        assert!(candidates.contains(&"今日".to_string()));
        assert!(candidates.contains(&"京".to_string()));

        // 最良候補が surface と一致
        assert_eq!(result.segments[0].surface, candidates[0]);
    }

    #[test]
    fn test_nbest_limit() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょう", &dict);
        let conn = SimpleConnectionCost;

        // n=1 では候補1つだけ
        let result = decode_nbest(&mut lattice, "きょう", &conn, 1);
        assert_eq!(result.segments[0].candidates.len(), 1);
    }

    #[test]
    fn test_nbest_ki() {
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("き", &dict);
        let conn = SimpleConnectionCost;
        let result = decode_nbest(&mut lattice, "き", &conn, 5);

        assert_eq!(result.segments.len(), 1);
        let candidates = &result.segments[0].candidates;

        // 「き」→「木」「気」の2候補
        assert!(candidates.len() >= 2, "2候補以上: {candidates:?}");
        assert!(candidates.contains(&"木".to_string()));
        assert!(candidates.contains(&"気".to_string()));
    }

    #[test]
    fn test_clauses_kyouha() {
        // 「きょうは」→ 単語: [今日, は] → 文節: [今日は]
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょうは", &dict);
        let conn = SimpleConnectionCost;
        let result = decode_nbest(&mut lattice, "きょうは", &conn, 5);

        // 単語レベルは2セグメント
        assert_eq!(result.segments.len(), 2);
        assert_eq!(result.segments[0].pos_type, POS_CONTENT); // 今日 = 名詞
        assert_eq!(result.segments[1].pos_type, POS_FUNCTION); // は = 助詞

        // 文節レベルは1文節
        assert_eq!(result.clauses.len(), 1, "1文節のはず: {:?}", result.clauses);
        assert_eq!(result.clauses[0].surface, "今日は");
        assert_eq!(result.clauses[0].yomi, "きょうは");
    }

    #[test]
    fn test_clauses_kyouha_tenki_desune() {
        // SimpleConnectionCost では「てんき」→「天」+「木」に分割される
        // 文節: [今日は] [天] [木ですね] (「木」は名詞=Content)
        let dict = HashMapDict::test_dict();
        let input = "きょうはてんきですね";
        let mut lattice = Lattice::build(input, &dict);
        let conn = SimpleConnectionCost;
        let result = decode_nbest(&mut lattice, input, &conn, 5);

        // 文節の読みが入力全体をカバー
        let all_yomi: String = result.clauses.iter().map(|c| c.yomi.as_str()).collect();
        assert_eq!(all_yomi, input);

        // 文節数 < 単語数 (付属語がまとまるため)
        assert!(
            result.clauses.len() < result.segments.len(),
            "文節数 ({}) < 単語数 ({}) のはず",
            result.clauses.len(),
            result.segments.len()
        );

        // 最初の文節は「今日は」
        assert_eq!(result.clauses[0].surface, "今日は");
    }

    #[test]
    fn test_clauses_candidates() {
        // 文節候補: 「きょうは」の文節候補に「京は」が含まれる
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょうは", &dict);
        let conn = SimpleConnectionCost;
        let result = decode_nbest(&mut lattice, "きょうは", &conn, 5);

        assert_eq!(result.clauses.len(), 1);
        let candidates = &result.clauses[0].candidates;
        assert!(
            candidates.contains(&"今日は".to_string()),
            "「今日は」が候補にあるはず: {candidates:?}"
        );
        assert!(
            candidates.contains(&"京は".to_string()),
            "「京は」が候補にあるはず: {candidates:?}"
        );
    }

    #[test]
    fn test_clauses_single_content() {
        // 自立語1つだけの場合は1文節
        let dict = HashMapDict::test_dict();
        let mut lattice = Lattice::build("きょう", &dict);
        let conn = SimpleConnectionCost;
        let result = decode_nbest(&mut lattice, "きょう", &conn, 5);

        assert_eq!(result.clauses.len(), 1);
        assert_eq!(result.clauses[0].surface, "今日");
    }
}
