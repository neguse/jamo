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
fn test_unknown_input_is_preserved() {
    let dict = HashMapDict::test_dict();
    let input = "ぴゃぴゃ";
    let mut lattice = Lattice::build(input, &dict);
    let conn = SimpleConnectionCost;
    let result = decode(&mut lattice, input, &conn);

    let combined_yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
    let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
    assert_eq!(combined_yomi, input);
    assert_eq!(text, input);
}

#[test]
fn test_mixed_known_unknown_input_is_preserved() {
    let dict = HashMapDict::test_dict();
    let input = "きょうぴゃ";
    let mut lattice = Lattice::build(input, &dict);
    let conn = SimpleConnectionCost;
    let result = decode(&mut lattice, input, &conn);

    let combined_yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
    let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
    assert_eq!(combined_yomi, input);
    assert_eq!(text, "今日ぴゃ");
}

#[test]
fn test_extreme_cumulative_cost_does_not_overflow() {
    struct ExtremeConnectionCost;

    impl ConnectionCost for ExtremeConnectionCost {
        fn cost(&self, _right_id: u16, _left_id: u16) -> i32 {
            i32::MAX
        }
    }

    let input = "あ";
    let mut lattice = Lattice {
        nodes: vec![
            crate::lattice::Node {
                start: 0,
                end: 0,
                surface: String::new(),
                left_id: 0,
                right_id: 0,
                word_cost: 0,
                pos_type: POS_CONTENT,
                total_cost: i64::MAX - 10,
                prev: None,
            },
            crate::lattice::Node {
                start: 0,
                end: input.len(),
                surface: input.to_string(),
                left_id: 0,
                right_id: 0,
                word_cost: 100,
                pos_type: POS_CONTENT,
                total_cost: i64::MAX,
                prev: None,
            },
            crate::lattice::Node {
                start: input.len(),
                end: input.len(),
                surface: String::new(),
                left_id: 0,
                right_id: 0,
                word_cost: 0,
                pos_type: POS_CONTENT,
                total_cost: i64::MAX,
                prev: None,
            },
        ],
        ends_at: {
            let mut ends_at = vec![Vec::new(); input.len() + 1];
            ends_at[0].push(0);
            ends_at[input.len()].extend([1, 2]);
            ends_at
        },
        input_len: input.len(),
    };

    let result = decode(&mut lattice, input, &ExtremeConnectionCost);
    assert!(result.segments.is_empty());
    assert_eq!(lattice.nodes[1].total_cost, i64::MAX);
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
fn test_nbest_candidates_ranked_by_context() {
    // 候補順は word_cost 単独ではなく、パス上の前後ノードとの接続コスト込みで決まる。
    // 「をかく」: を(助詞) の後ろでは動詞が名詞より接続コストで有利になる設定。
    // word_cost 順: 核(3000) < 欠く(4500) < 書く(5000)
    // 文脈込み:     書く(Viterbi最良) < 欠く < 核
    struct TestConn;
    impl ConnectionCost for TestConn {
        fn cost(&self, right_id: u16, left_id: u16) -> i32 {
            match (right_id, left_id) {
                (3, 2) => -4000, // を → 書く
                (3, 4) => -2000, // を → 欠く
                _ => 0,
            }
        }
    }

    let mut dict = HashMapDict::new();
    dict.add("てがみ", "手紙", 1, 1, 1000, POS_CONTENT);
    dict.add("を", "を", 3, 3, 500, POS_FUNCTION);
    dict.add("かく", "核", 1, 1, 3000, POS_CONTENT);
    dict.add("かく", "書く", 2, 2, 5000, POS_CONTENT);
    dict.add("かく", "欠く", 4, 4, 4500, POS_CONTENT);

    let input = "てがみをかく";
    let mut lattice = Lattice::build(input, &dict);
    let result = decode_nbest(&mut lattice, input, &TestConn, 5);

    let kaku = result
        .segments
        .iter()
        .find(|s| s.yomi == "かく")
        .expect("「かく」セグメントがあるはず");
    assert_eq!(
        kaku.candidates,
        vec!["書く", "欠く", "核"],
        "文脈込みコスト順のはず"
    );
}

#[test]
fn test_nbest_candidates_dedupe_same_surface() {
    // 同一表層で品詞違いのエントリが候補に重複して並ばないこと
    // (コスト順で間に別表層が挟まると隣接 dedup では取れない)
    let mut dict = HashMapDict::new();
    dict.add("ほん", "本", 1, 1, 5000, POS_CONTENT);
    dict.add("ほん", "ホン", 2, 2, 5500, POS_CONTENT);
    dict.add("ほん", "保温", 3, 3, 5700, POS_CONTENT);
    dict.add("ほん", "ホン", 4, 4, 6000, POS_CONTENT); // 別品詞の同一表層

    let input = "ほん";
    let mut lattice = Lattice::build(input, &dict);
    let result = decode_nbest(&mut lattice, input, &SimpleConnectionCost, 5);

    let candidates = &result.segments[0].candidates;
    let hon_count = candidates.iter().filter(|c| c.as_str() == "ホン").count();
    assert_eq!(hon_count, 1, "「ホン」は1回だけのはず: {candidates:?}");
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
fn test_clauses_kana_fallback_candidates() {
    // 文節候補には読みのひらがな/カタカナが常に含まれる (辞書にない語の救済)
    let dict = HashMapDict::test_dict();
    let mut lattice = Lattice::build("きょうは", &dict);
    let result = decode_nbest(&mut lattice, "きょうは", &SimpleConnectionCost, 5);

    assert_eq!(result.clauses.len(), 1);
    let candidates = &result.clauses[0].candidates;
    assert!(
        candidates.contains(&"きょうは".to_string()),
        "ひらがな候補があるはず: {candidates:?}"
    );
    assert!(
        candidates.contains(&"キョウハ".to_string()),
        "カタカナ候補があるはず: {candidates:?}"
    );
}

#[test]
fn test_clauses_kana_fallback_no_duplicates() {
    // 表層が読みと同じ文節 (です) でもひらがな候補が重複しないこと
    let dict = HashMapDict::test_dict();
    let mut lattice = Lattice::build("です", &dict);
    let result = decode_nbest(&mut lattice, "です", &SimpleConnectionCost, 5);

    assert_eq!(result.clauses.len(), 1);
    let candidates = &result.clauses[0].candidates;
    let count = candidates.iter().filter(|c| c.as_str() == "です").count();
    assert_eq!(count, 1, "「です」は1回だけのはず: {candidates:?}");
}

#[test]
fn test_convert_with_boundaries_kana_fallback() {
    // 境界指定変換でも文節候補にかなフォールバックが含まれる
    let dict = HashMapDict::test_dict();
    let input = "きょうは";
    let boundaries = vec![9, 12];
    let result = convert_with_boundaries(input, &boundaries, &dict, &SimpleConnectionCost, 5);

    assert_eq!(result.clauses.len(), 2);
    let candidates = &result.clauses[0].candidates;
    assert!(
        candidates.contains(&"きょう".to_string()),
        "ひらがな候補があるはず: {candidates:?}"
    );
    assert!(
        candidates.contains(&"キョウ".to_string()),
        "カタカナ候補があるはず: {candidates:?}"
    );
}

#[test]
fn test_convert_with_boundaries_full_span_candidates() {
    // 複数の内部文節を含む境界区間では、候補は部分スパンの文字列ではなく
    // 区間全体を置換した形で並ぶ (部分候補を選ぶと残りが消えるバグの回帰テスト)
    let dict = HashMapDict::test_dict();
    let input = "きょうはてんき";
    let boundaries = vec![input.len()];
    let result = convert_with_boundaries(input, &boundaries, &dict, &SimpleConnectionCost, 5);

    assert_eq!(result.clauses.len(), 1);
    // SimpleConnectionCost では「てんき」→「天」+「木」が最良
    assert_eq!(result.clauses[0].surface, "今日は天木");
    let candidates = &result.clauses[0].candidates;
    assert!(
        candidates.contains(&"京は天木".to_string()),
        "全スパン置換の候補があるはず: {candidates:?}"
    );
    assert!(
        candidates.contains(&"今日は天気".to_string()),
        "内部文節の代替も全スパン形のはず: {candidates:?}"
    );
    assert!(
        !candidates.iter().any(|c| c == "京は"),
        "部分スパン候補が混入しないはず: {candidates:?}"
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

#[test]
fn test_resize_extend() {
    // 「きょうは」(12 bytes) を境界 [9, 12] (きょう|は) で分割
    // clause 0 を 1文字伸ばす → 次の文節「は」(1文字) を完全吸収 → [12] (1文節)
    let input = "きょうは"; // 12 bytes
    let boundaries = vec![9, 12]; // きょう(9) | は(3)
    let result = resize_boundaries(input, &boundaries, 0, 1);
    assert_eq!(result, Some(vec![12]));
}

#[test]
fn test_resize_shrink() {
    // 「きょうは」境界 [12] (1文節) を 1文字縮める
    // → [9, 12] (きょう|は)
    let input = "きょうは"; // 12 bytes
    let boundaries = vec![9, 12]; // きょう | は
    let result = resize_boundaries(input, &boundaries, 0, -1);
    // 「きょう」→ 3文字, -1 で 2文字 = 6 bytes (きょ)
    assert_eq!(result, Some(vec![6, 12]));
}

#[test]
fn test_resize_boundary_limits() {
    let input = "きょうは"; // 12 bytes
    // 1文字文節は縮められない (なくなるため)
    let boundaries = vec![3, 12]; // き | ょうは
    let result = resize_boundaries(input, &boundaries, 0, -1);
    assert_eq!(result, None);

    // 最後の文節は伸縮不可
    let boundaries = vec![9, 12];
    let result = resize_boundaries(input, &boundaries, 1, 1);
    assert_eq!(result, None);
}

#[test]
fn test_validate_boundaries_rejects_invalid_offsets() {
    let input = "きょうは";
    assert!(matches!(
        validate_boundaries(input, &[1, input.len()]),
        Err(BoundaryError::NotCharBoundary { .. })
    ));
    assert!(matches!(
        validate_boundaries(input, &[9]),
        Err(BoundaryError::MissingInputEnd { .. })
    ));
    assert!(matches!(
        validate_boundaries(input, &[12, 9]),
        Err(BoundaryError::NotStrictlyAscending { .. })
    ));
}

#[test]
fn test_resize_rejects_invalid_boundaries_without_panic() {
    let input = "きょうは";
    assert_eq!(resize_boundaries(input, &[1, input.len()], 0, 1), None);
}

#[test]
fn test_convert_with_boundaries() {
    // 「きょうは」を文節境界 [9, 12] で変換 → [今日][は]
    let dict = HashMapDict::test_dict();
    let input = "きょうは";
    let boundaries = vec![9, 12];
    let conn = SimpleConnectionCost;
    let result = convert_with_boundaries(input, &boundaries, &dict, &conn, 5);

    assert_eq!(result.clauses.len(), 2);
    assert_eq!(result.clauses[0].yomi, "きょう");
    assert_eq!(result.clauses[0].surface, "今日");
    assert_eq!(result.clauses[1].yomi, "は");
    assert_eq!(result.clauses[1].surface, "は");
}

#[test]
fn test_try_convert_with_boundaries_rejects_truncated_input() {
    let dict = HashMapDict::test_dict();
    let input = "きょうは";
    let conn = SimpleConnectionCost;
    let result = try_convert_with_boundaries(input, &[9], &dict, &conn, 5);
    assert!(matches!(result, Err(BoundaryError::MissingInputEnd { .. })));
}

#[test]
fn test_resize_and_reconvert() {
    // フル変換 → 伸縮 → 再変換 のフロー
    let dict = HashMapDict::test_dict();
    let input = "きょうは";
    let conn = SimpleConnectionCost;

    // 初回変換: 自動文節分割
    let result = decode_nbest(&mut Lattice::build(input, &dict), input, &conn, 5);

    // 文節境界を取得
    let mut boundaries: Vec<usize> = Vec::new();
    let mut pos = 0;
    for clause in &result.clauses {
        pos += clause.yomi.len();
        boundaries.push(pos);
    }
    assert_eq!(*boundaries.last().unwrap(), input.len());

    // 文節 0 を縮める
    if boundaries.len() > 1 {
        if let Some(new_boundaries) = resize_boundaries(input, &boundaries, 0, -1) {
            let new_result = convert_with_boundaries(input, &new_boundaries, &dict, &conn, 5);

            // 読みが入力全体をカバー
            let all_yomi: String = new_result.clauses.iter().map(|c| c.yomi.as_str()).collect();
            assert_eq!(all_yomi, input);
        }
    }
}
