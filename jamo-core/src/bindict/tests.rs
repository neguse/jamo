use super::*;
use crate::dict::{POS_CONTENT, POS_FUNCTION};

fn e(surface: &str, left: u16, right: u16, cost: i16, pos: u8) -> Entry {
    Entry {
        surface: surface.to_string(),
        left_id: left,
        right_id: right,
        cost,
        pos_type: pos,
    }
}

/// HashMapDict::test_dict 相当 + 恒等表層とエスケープ文字を含む辞書
fn sample_keys() -> Vec<(String, Vec<Entry>)> {
    vec![
        (
            "きょう".into(),
            vec![
                e("今日", 0, 0, -3000, POS_CONTENT),
                e("京", 0, 0, -2000, POS_CONTENT),
                e("きょう", 5, 5, 100, POS_CONTENT),
                e("キョウ", 5, 5, 200, POS_CONTENT),
            ],
        ),
        ("き".into(), vec![e("木", 0, 0, -2500, POS_CONTENT)]),
        ("は".into(), vec![e("は", 1, 1, -3000, POS_FUNCTION)]),
        ("いい".into(), vec![e("良い", 2, 2, -2500, POS_CONTENT)]),
        ("てんき".into(), vec![e("天気", 0, 0, -5000, POS_CONTENT)]),
        ("てん".into(), vec![e("天", 0, 0, -2000, POS_CONTENT)]),
        ("です".into(), vec![e("です", 4, 4, -3000, POS_FUNCTION)]),
        // 恒等表層でも surface のカタカナが読みの katakana と違えばプール行き
        ("げーむ".into(), vec![e("ゲーム", 0, 0, 10, POS_CONTENT)]),
        // 混在表層 (ひらがな+カタカナ) はプール
        ("あい".into(), vec![e("あイ", 0, 0, 10, POS_CONTENT)]),
    ]
}

fn sample_matrix() -> (Vec<i16>, usize) {
    // 6x6、行ごとにデフォルト値 + 例外を混ぜる
    let n = 6;
    let mut m = vec![0i16; n * n];
    for r in 0..n {
        for l in 0..n {
            m[r * n + l] = (r as i16) * 100;
        }
    }
    m[1] = -500;
    m[2 * n + 3] = 42;
    m[5 * n + 5] = i16::MIN;
    m[5 * n] = i16::MAX;
    (m, n)
}

fn sample_dict() -> BinDict {
    let keys = sample_keys();
    let (matrix, num_pos) = sample_matrix();
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &matrix,
        num_pos,
        boundary: &[],
    })
    .unwrap();
    let dict = BinDict::from_vec(bytes).unwrap();
    dict.verify().unwrap();
    dict
}

fn surfaces(results: &[(usize, Vec<Entry>)], len: usize) -> Vec<String> {
    results
        .iter()
        .filter(|(l, _)| *l == len)
        .flat_map(|(_, es)| es.iter().map(|e| e.surface.clone()))
        .collect()
}

#[test]
fn test_common_prefix_search_roundtrip() {
    let dict = sample_dict();
    let results = dict.common_prefix_search("きょうは");
    let lengths: Vec<usize> = results.iter().map(|(l, _)| *l).collect();
    assert_eq!(lengths, vec![3, 9]);
    assert_eq!(surfaces(&results, 3), vec!["木"]);
    assert_eq!(
        surfaces(&results, 9),
        vec!["今日", "京", "きょう", "キョウ"]
    );

    let kyou = &results[1].1;
    assert_eq!(kyou[0].left_id, 0);
    assert_eq!(kyou[0].cost, -3000);
    assert_eq!(kyou[2].left_id, 5);
    assert_eq!(kyou[2].right_id, 5);
    assert_eq!(kyou[3].cost, 200);
    assert_eq!(kyou[0].pos_type, POS_CONTENT);

    let results = dict.common_prefix_search("ですね");
    assert_eq!(surfaces(&results, 6), vec!["です"]);
    assert_eq!(results[0].1[0].pos_type, POS_FUNCTION);
}

#[test]
fn test_identity_and_pool_surfaces() {
    let dict = sample_dict();
    assert_eq!(
        surfaces(&dict.common_prefix_search("げーむ"), 9),
        vec!["ゲーム"]
    );
    assert_eq!(
        surfaces(&dict.common_prefix_search("あい"), 6),
        vec!["あイ"]
    );
    let (_, _, combos, pool_surfaces, _) = dict.counts();
    // プール: 今日 京 木 良い 天気 天 あイ (ゲーム は katakana 恒等)
    assert_eq!(pool_surfaces, 7);
    // (left,right,pos) の組: (0,0,C) (5,5,C) (1,1,F) (2,2,C) (4,4,F)
    assert_eq!(combos, 5);
}

#[test]
fn test_no_match_and_unknown_chars() {
    let dict = sample_dict();
    assert!(dict.common_prefix_search("").is_empty());
    assert!(dict.common_prefix_search("zzz").is_empty());
    // 表にない文字 (エスケープ経路) でも panic しない
    assert!(dict.common_prefix_search("漢字").is_empty());
    let results = dict.common_prefix_search("き漢");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 3);
}

#[test]
fn test_connection_cost_defaults_and_exceptions() {
    let dict = sample_dict();
    let (matrix, n) = sample_matrix();
    for r in 0..n {
        for l in 0..n {
            assert_eq!(
                dict.connection_cost(r as u16, l as u16),
                matrix[r * n + l],
                "cost({r},{l})"
            );
        }
    }
    assert_eq!(dict.connection_cost(6, 0), 0);
    assert_eq!(dict.connection_cost(0, 100), 0);
    assert_eq!(dict.pos_size(), 6);
}

#[test]
fn test_stats() {
    let dict = sample_dict();
    assert_eq!(dict.vocab_size(), 12);
    assert_eq!(dict.reading_count(), 9);
    let sections = dict.sections();
    assert_eq!(sections.len(), SECTION_COUNT);
    let total: usize = sections.iter().map(|s| s.bytes).sum();
    assert!(total <= dict.len());
}

#[test]
fn test_prefix_search() {
    let dict = sample_dict();
    let all: Vec<String> = dict
        .prefix_search("て", 100)
        .into_iter()
        .map(|e| e.surface)
        .collect();
    assert_eq!(all, vec!["天", "天気"]);
    let limited = dict.prefix_search("き", 1);
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].surface, "木");
    assert!(dict.prefix_search("ぬ", 10).is_empty());
    // 恒等表層は DFS 中に復元した読みから作る
    let kyou: Vec<String> = dict
        .prefix_search("きょ", 10)
        .into_iter()
        .map(|e| e.surface)
        .collect();
    assert_eq!(kyou, vec!["今日", "京", "きょう", "キョウ"]);
}

#[test]
fn test_convert_through_engine() {
    let dict = sample_dict();
    let result = crate::convert_with_conn("きょうはいいてんきですね", &dict, &dict);
    let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
    assert_eq!(text, "今日は良い天気ですね");
}

#[test]
fn test_empty_dict() {
    let keys: Vec<(String, Vec<Entry>)> = Vec::new();
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &[0],
        num_pos: 1,
        boundary: &[],
    })
    .unwrap();
    let dict = BinDict::from_vec(bytes).unwrap();
    dict.verify().unwrap();
    assert!(dict.common_prefix_search("きょう").is_empty());
    assert!(dict.prefix_search("", 10).is_empty());
    assert_eq!(dict.vocab_size(), 0);
}

#[test]
fn test_build_rejects_empty_surface() {
    let keys = vec![("あ".to_string(), vec![e("", 0, 0, 0, POS_CONTENT)])];
    assert!(
        build(BuildInput {
            keys: &keys,
            matrix: &[0],
            num_pos: 1,
            boundary: &[],
        })
        .is_err()
    );
}

#[test]
fn test_build_rejects_bad_matrix() {
    let keys = sample_keys();
    assert!(
        build(BuildInput {
            keys: &keys,
            matrix: &[0; 3],
            num_pos: 2,
            boundary: &[],
        })
        .is_err()
    );
    assert!(
        build(BuildInput {
            keys: &keys,
            matrix: &[],
            num_pos: 0,
            boundary: &[],
        })
        .is_err()
    );
}

#[test]
fn test_open_rejects_corruption() {
    let keys = sample_keys();
    let (matrix, num_pos) = sample_matrix();
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &matrix,
        num_pos,
        boundary: &[],
    })
    .unwrap();

    // 切り詰め
    for cut in [0, 4, 8, HEADER_LEN - 1, HEADER_LEN + 10, bytes.len() - 1] {
        let err = BinDict::from_vec(bytes[..cut].to_vec()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData, "cut {cut}");
    }
    // magic / version
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert!(BinDict::from_vec(bad).is_err());
    let mut bad = bytes.clone();
    bad[4] = 2;
    assert!(BinDict::from_vec(bad).is_err());
    // セクションの重なり (labels を louds と同じ位置に)
    let mut bad = bytes.clone();
    let off = read_u32(&bytes, 16 + SEC_LOUDS * 8);
    bad[16 + SEC_LABELS * 8..20 + SEC_LABELS * 8].copy_from_slice(&off.to_le_bytes());
    assert!(BinDict::from_vec(bad).is_err());
    // labels の長さがノード数と合わない
    let mut bad = bytes.clone();
    let len = read_u32(&bytes, 20 + SEC_LABELS * 8) - 1;
    bad[20 + SEC_LABELS * 8..24 + SEC_LABELS * 8].copy_from_slice(&len.to_le_bytes());
    assert!(BinDict::from_vec(bad).is_err());
    // 末尾のゴミは TOC 外なので許容される (ヘッダ経由でしか参照しない)
    let mut padded = bytes.clone();
    padded.push(0);
    assert!(BinDict::from_vec(padded).is_ok());
}

#[test]
fn test_verify_detects_bad_references() {
    let keys = sample_keys();
    let (matrix, num_pos) = sample_matrix();
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &matrix,
        num_pos,
        boundary: &[],
    })
    .unwrap();
    let dict = BinDict::from_vec(bytes.clone()).unwrap();
    let tokens = dict.layout.sections[SEC_TOKENS].clone();
    let ids = dict.layout.sections[SEC_SURFACE_IDS].clone();

    // combo 番号を範囲外に
    let mut bad = bytes.clone();
    bad[tokens.start] = 0xFF;
    bad[tokens.start + 1] = 0x3F;
    let d = BinDict::from_vec(bad).unwrap();
    assert!(d.verify().is_err());
    // 範囲外でも検索は panic せず打ち切る
    let _ = d.common_prefix_search("きょうはいいてんきですね");

    // 表層 id を範囲外に
    let mut bad = bytes.clone();
    bad[ids.start + 2] = 0xFF;
    let d = BinDict::from_vec(bad).unwrap();
    assert!(d.verify().is_err());
    let _ = d.common_prefix_search("きょう");
}

#[test]
fn test_file_roundtrip_via_mmap() {
    let keys = sample_keys();
    let (matrix, num_pos) = sample_matrix();
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &matrix,
        num_pos,
        boundary: &[],
    })
    .unwrap();
    let path = std::env::temp_dir().join("jamo_bindict_mmap_test.dict");
    std::fs::write(&path, &bytes).unwrap();
    let dict = BinDict::open(&path).unwrap();
    dict.verify().unwrap();
    assert_eq!(dict.len(), bytes.len());
    assert_eq!(
        surfaces(&dict.common_prefix_search("てんき"), 9),
        vec!["天気"]
    );
    let dict = BinDict::read(&path).unwrap();
    assert_eq!(dict.vocab_size(), 12);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_boundary_penalty_changes_single_word_result() {
    // 「いく」: 非自立動詞 (id 3、BOS→3 = 0、3→EOS = 300) のコスト 0 が自立動詞
    // 「行く」(id 0、接続 0、コスト 500) に勝つ状況を PREFIX ペナルティで逆転させる
    // (mozc の boundary.def と同じ仕組み)
    let keys = vec![(
        "いく".to_string(),
        vec![
            e("いく", 3, 3, 0, POS_FUNCTION),
            e("行く", 0, 0, 500, POS_CONTENT),
        ],
    )];
    let (matrix, num_pos) = sample_matrix();
    let mut boundary = vec![(0u16, 0u16); num_pos];
    boundary[3] = (4000, 300);
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &matrix,
        num_pos,
        boundary: &boundary,
    })
    .unwrap();
    let dict = BinDict::from_vec(bytes).unwrap();
    dict.verify().unwrap();
    assert_eq!(dict.prefix_penalty(3), 4000);
    assert_eq!(dict.suffix_penalty(3), 300);
    assert_eq!(dict.prefix_penalty(0), 0);
    assert_eq!(dict.prefix_penalty(99), 0);
    let result = crate::convert_with_conn("いく", &dict, &dict);
    assert_eq!(result.segments[0].surface, "行く");

    // 表なし (空) なら 0 で、コスト順どおり「いく」
    let bytes = build(BuildInput {
        keys: &keys,
        matrix: &matrix,
        num_pos,
        boundary: &[],
    })
    .unwrap();
    let dict = BinDict::from_vec(bytes).unwrap();
    assert_eq!(dict.prefix_penalty(3), 0);
    let result = crate::convert_with_conn("いく", &dict, &dict);
    assert_eq!(result.segments[0].surface, "いく");

    // サイズ不一致はビルドエラー、壊れたセクション長はオープンエラー
    assert!(
        build(BuildInput {
            keys: &keys,
            matrix: &matrix,
            num_pos,
            boundary: &boundary[..2],
        })
        .is_err()
    );
}
