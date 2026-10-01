use super::*;
use crate::dict::Dictionary;

#[test]
fn test_katakana_to_hiragana() {
    assert_eq!(katakana_to_hiragana("キョウ"), "きょう");
    assert_eq!(katakana_to_hiragana("テンキ"), "てんき");
    assert_eq!(katakana_to_hiragana("ハ"), "は");
}

#[test]
fn test_parse_csv_line() {
    let line = "今日,1285,1285,5543,名詞,一般,*,*,*,*,今日,キョウ,キョー";
    let entry = parse_csv_line(line).unwrap();
    assert_eq!(entry.surface, "今日");
    assert_eq!(entry.left_id, 1285);
    assert_eq!(entry.right_id, 1285);
    assert_eq!(entry.cost, 5543);
    assert_eq!(entry.reading, "キョウ");
}

#[test]
fn test_parse_matrix() {
    let content = "3 3\n0 0 -100\n0 1 200\n1 0 -50\n2 2 300\n";
    let (n, matrix) = parse_matrix(content).unwrap();
    assert_eq!(n, 3);
    assert_eq!(matrix[0 * 3 + 0], -100);
    assert_eq!(matrix[0 * 3 + 1], 200);
    assert_eq!(matrix[1 * 3 + 0], -50);
    assert_eq!(matrix[2 * 3 + 2], 300);
}

#[test]
fn test_convert_with_ipadic() {
    let ipadic_dir = Path::new("../data/ipadic");
    if !ipadic_dir.exists() {
        eprintln!("Skipping: IPAdic not found");
        return;
    }

    let dict = IpaDict::load(ipadic_dir)
        .expect("failed to load IPAdic")
        .into_bindict()
        .expect("failed to build");
    let result = crate::convert_with_conn("きょうはいいてんきですね", &dict, &dict);

    let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
    eprintln!("変換結果: {text}");

    // 最低限、入力全体をカバーしているはず
    let yomi: String = result.segments.iter().map(|s| s.yomi.as_str()).collect();
    assert_eq!(yomi, "きょうはいいてんきですね");

    // 接続コスト付きで「天気」が出るようになった (Iter 1 では「天木」だった)
    assert!(text.contains("天気"), "「天気」が含まれるはず: {text}");
}

#[test]
fn test_load_ipadic() {
    let ipadic_dir = Path::new("../data/ipadic");
    if !ipadic_dir.exists() {
        eprintln!("Skipping: IPAdic not found at {}", ipadic_dir.display());
        return;
    }

    let dict = IpaDict::load(ipadic_dir).expect("failed to load IPAdic");
    eprintln!(
        "Loaded {} vocab, {} readings, {} pos",
        dict.vocab_size(),
        dict.reading_count(),
        dict.pos_size()
    );

    assert!(
        dict.vocab_size() > 300_000,
        "IPAdic should have >300k entries"
    );
    assert!(dict.pos_size() > 1000, "IPAdic should have >1000 POS IDs");

    // 「きょう」で引けるはず
    let dict = dict.into_bindict().expect("failed to build");
    let results = dict.common_prefix_search("きょう");
    assert!(!results.is_empty(), "「きょう」がヒットするはず");

    let surfaces: Vec<&str> = results
        .iter()
        .flat_map(|(_, entries)| entries.iter().map(|e| e.surface.as_str()))
        .collect();
    assert!(
        surfaces.contains(&"今日"),
        "「今日」が候補にあるはず: {:?}",
        surfaces
    );
}

#[test]
fn test_clauses_with_ipadic() {
    let ipadic_dir = Path::new("../data/ipadic");
    if !ipadic_dir.exists() {
        eprintln!("Skipping: IPAdic not found");
        return;
    }

    let dict = IpaDict::load(ipadic_dir)
        .expect("failed to load IPAdic")
        .into_bindict()
        .expect("failed to build");
    let result = crate::convert_nbest("わたしはがくせいです", &dict, &dict, 5);

    let clause_surfaces: Vec<&str> = result.clauses.iter().map(|c| c.surface.as_str()).collect();
    let clause_yomis: Vec<&str> = result.clauses.iter().map(|c| c.yomi.as_str()).collect();
    eprintln!("文節: {clause_surfaces:?}");
    eprintln!("読み: {clause_yomis:?}");

    // 読みが入力全体をカバー
    let all_yomi: String = result.clauses.iter().map(|c| c.yomi.as_str()).collect();
    assert_eq!(all_yomi, "わたしはがくせいです");

    // 文節数は単語数より少ないはず (助詞・助動詞がまとまる)
    assert!(
        result.clauses.len() < result.segments.len(),
        "文節数 ({}) < 単語数 ({}) のはず",
        result.clauses.len(),
        result.segments.len()
    );
}

#[test]
fn test_classify_pos() {
    use crate::dict::{POS_CONTENT, POS_FUNCTION};
    assert_eq!(classify_pos("名詞", "一般"), POS_CONTENT);
    assert_eq!(classify_pos("名詞", "接尾"), POS_FUNCTION);
    assert_eq!(classify_pos("名詞", "非自立"), POS_FUNCTION);
    assert_eq!(classify_pos("動詞", "自立"), POS_CONTENT);
    assert_eq!(classify_pos("動詞", "非自立"), POS_FUNCTION);
    assert_eq!(classify_pos("動詞", "接尾"), POS_FUNCTION);
    assert_eq!(classify_pos("形容詞", "自立"), POS_CONTENT);
    assert_eq!(classify_pos("形容詞", "非自立"), POS_FUNCTION);
    assert_eq!(classify_pos("助詞", "格助詞"), POS_FUNCTION);
    assert_eq!(classify_pos("助動詞", "*"), POS_FUNCTION);
    assert_eq!(classify_pos("副詞", "一般"), POS_CONTENT);
    assert_eq!(classify_pos("接続詞", "*"), POS_CONTENT);
    assert_eq!(classify_pos("接頭詞", "名詞接続"), POS_CONTENT);
}

#[test]
fn test_load_merge() {
    let ipadic_dir = Path::new("../data/ipadic");
    if !ipadic_dir.exists() {
        eprintln!("Skipping: IPAdic not found");
        return;
    }

    // ベースの辞書をロード
    let base = IpaDict::load(ipadic_dir).expect("failed to load base");
    let base_vocab = base.vocab_size();

    // 追加CSVを一時ファイルに書き出し
    let tmp_dir = std::env::temp_dir().join("jamo_merge_test");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let extra_csv = tmp_dir.join("extra.csv");
    std::fs::write(
            &extra_csv,
            // NEologd風のエントリ (IPAdicと同じCSV形式)
            "スマートフォン,1285,1285,3000,名詞,一般,*,*,*,*,スマートフォン,スマートフォン,スマートフォン\n\
             クラウドファンディング,1285,1285,4000,名詞,一般,*,*,*,*,クラウドファンディング,クラウドファンディング,クラウドファンディング\n",
        )
        .unwrap();

    // マージしてロード
    let merged = IpaDict::load_merge(ipadic_dir, &[extra_csv.as_path()]).expect("failed to merge");
    let merged_pos = merged.pos_size();
    let merged_conn = merged.connection_cost(0, 0);
    let merged_vocab = merged.vocab_size();
    let merged = merged.into_bindict().expect("failed to build");

    // 追加エントリ分だけ語彙が増えている
    assert!(
        merged_vocab > base_vocab,
        "merged {merged_vocab} should be > base {base_vocab}"
    );
    assert_eq!(merged.vocab_size(), merged_vocab);

    // 追加したエントリが検索可能
    let results = merged.common_prefix_search("すまーとふぉん");
    let surfaces: Vec<&str> = results
        .iter()
        .flat_map(|(_, entries)| entries.iter().map(|e| e.surface.as_str()))
        .collect();
    assert!(
        surfaces.contains(&"スマートフォン"),
        "「スマートフォン」が候補にあるはず: {:?}",
        surfaces
    );

    // ベースのエントリも引き続き検索可能
    let results = merged.common_prefix_search("きょう");
    let surfaces: Vec<&str> = results
        .iter()
        .flat_map(|(_, entries)| entries.iter().map(|e| e.surface.as_str()))
        .collect();
    assert!(
        surfaces.contains(&"今日"),
        "「今日」がベースから引き続きあるはず: {:?}",
        surfaces
    );

    // 接続コスト行列はベースと同一
    assert_eq!(merged_pos, base.pos_size());
    assert_eq!(merged_conn, base.connection_cost(0, 0));
    assert_eq!(merged.connection_cost(0, 0), base.connection_cost(0, 0));

    // クリーンアップ
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_load_merge_directory() {
    let ipadic_dir = Path::new("../data/ipadic");
    if !ipadic_dir.exists() {
        eprintln!("Skipping: IPAdic not found");
        return;
    }

    // 追加CSVディレクトリを作成
    let tmp_dir = std::env::temp_dir().join("jamo_merge_dir_test");
    let _ = std::fs::create_dir_all(&tmp_dir);
    std::fs::write(
            tmp_dir.join("neologd.csv"),
            "ブロックチェーン,1285,1285,3500,名詞,一般,*,*,*,*,ブロックチェーン,ブロックチェーン,ブロックチェーン\n",
        )
        .unwrap();

    // ディレクトリごとマージ
    let merged = IpaDict::load_merge(ipadic_dir, &[tmp_dir.as_path()])
        .expect("failed to merge dir")
        .into_bindict()
        .expect("failed to build");

    let results = merged.common_prefix_search("ぶろっくちぇーん");
    let surfaces: Vec<&str> = results
        .iter()
        .flat_map(|(_, entries)| entries.iter().map(|e| e.surface.as_str()))
        .collect();
    assert!(
        surfaces.contains(&"ブロックチェーン"),
        "ディレクトリからのマージで候補にあるはず: {:?}",
        surfaces
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_binary_roundtrip() {
    let ipadic_dir = Path::new("../data/ipadic");
    if !ipadic_dir.exists() {
        eprintln!("Skipping: IPAdic not found");
        return;
    }

    let dict = IpaDict::load(ipadic_dir).expect("failed to load IPAdic");

    // バイナリに書き出し
    let tmp = std::env::temp_dir().join("jamo_test_dict.bin");
    dict.save_binary(&tmp).expect("failed to save binary");

    let file_size = fs::metadata(&tmp).unwrap().len();
    eprintln!(
        "Binary dict size: {} bytes ({:.1} MB)",
        file_size,
        file_size as f64 / 1_048_576.0
    );

    // mmap で開いたものと、メモリ上で構築したものが一致する
    let dict2 = BinDict::open(&tmp).expect("failed to open binary");
    dict2.verify().expect("binary should verify");
    let dict1 = dict.into_bindict().expect("failed to build");

    assert_eq!(dict1.vocab_size(), dict2.vocab_size());
    assert_eq!(dict1.reading_count(), dict2.reading_count());
    assert_eq!(dict1.pos_size(), dict2.pos_size());

    let result1 = crate::convert_with_conn("わたしはがくせいです", &dict1, &dict1);
    let result2 = crate::convert_with_conn("わたしはがくせいです", &dict2, &dict2);
    let text1: String = result1
        .segments
        .iter()
        .map(|s| s.surface.as_str())
        .collect();
    let text2: String = result2
        .segments
        .iter()
        .map(|s| s.surface.as_str())
        .collect();
    assert_eq!(text1, text2);

    assert_eq!(dict1.connection_cost(0, 0), dict2.connection_cost(0, 0));
    assert_eq!(
        dict1.connection_cost(100, 200),
        dict2.connection_cost(100, 200)
    );

    let _ = fs::remove_file(&tmp);
}
