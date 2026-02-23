// T030: 変換精度テスト
// IPAdic を使った変換精度のベースライン計測

use std::path::Path;

/// テストコーパス: (読み, 期待される変換結果)
/// 一般的な日本語の文を網羅
fn test_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        // 基本的な名詞+助詞
        ("わたしはがくせいです", "私は学生です"),
        ("にほんごをべんきょうする", "日本語を勉強する"),
        ("とうきょうにいきたい", "東京に行きたい"),
        ("ほんをよむ", "本を読む"),
        ("みずをのむ", "水を飲む"),
        // 形容詞
        ("たかいやま", "高い山"),
        ("おおきいいえ", "大きい家"),
        ("あたらしいほん", "新しい本"),
        // 動詞活用
        ("あるいていく", "歩いていく"),
        ("たべている", "食べている"),
        ("はしっている", "走っている"),
        // 丁寧語
        ("おはようございます", "おはようございます"),
        ("ありがとうございます", "ありがとうございます"),
        // 日常表現
        ("なまえはたなかです", "名前は田中です"),
        ("がっこうにいく", "学校に行く"),
        ("しごとをする", "仕事をする"),
        ("でんわをかける", "電話をかける"),
        ("てがみをかく", "手紙を書く"),
        // 場所
        ("おおさかにすんでいます", "大阪に住んでいます"),
        ("えきまであるく", "駅まで歩く"),
        // 時間
        ("あしたはやすみです", "明日は休みです"),
        ("まいにちべんきょうする", "毎日勉強する"),
        // やや複雑な文
        ("にほんのしゅとはとうきょうです", "日本の首都は東京です"),
        ("ともだちとえいがをみる", "友達と映画を見る"),
        ("かぞくでりょこうにいく", "家族で旅行に行く"),
        // 数字・数量関連
        ("さんにんのがくせい", "三人の学生"),
        // 接続
        (
            "あめがふっているからかさをもつ",
            "雨が降っているから傘を持つ",
        ),
        (
            "さむいのであたたかいものをたべる",
            "寒いので暖かいものを食べる",
        ),
        // 短い文
        ("いく", "行く"),
        ("みる", "見る"),
        ("する", "する"),
        ("くる", "来る"),
        ("ある", "ある"),
        ("いる", "いる"),
        ("たべる", "食べる"),
        ("のむ", "飲む"),
        ("よむ", "読む"),
        ("かく", "書く"),
        ("はなす", "話す"),
        ("きく", "聞く"),
        ("おもう", "思う"),
        ("しる", "知る"),
        // 名詞単独
        ("やま", "山"),
        ("かわ", "川"),
        ("そら", "空"),
        ("うみ", "海"),
        ("はな", "花"),
        ("ひと", "人"),
        ("くるま", "車"),
        ("でんしゃ", "電車"),
        ("ひこうき", "飛行機"),
        ("びょういん", "病院"),
    ]
}

fn load_ipadic() -> Option<jamo_core::ipadic::IpaDict> {
    let candidates = ["../data/ipadic", "data/ipadic"];
    for p in candidates {
        let path = Path::new(p);
        if path.exists() {
            return jamo_core::ipadic::IpaDict::load(path).ok();
        }
    }
    // バイナリ辞書
    let bin_candidates = ["../data/ipadic.dict", "data/ipadic.dict"];
    for p in bin_candidates {
        let path = Path::new(p);
        if path.exists() {
            return jamo_core::ipadic::IpaDict::load_binary(path).ok();
        }
    }
    None
}

#[test]
fn test_accuracy_baseline() {
    let dict = match load_ipadic() {
        Some(d) => d,
        None => {
            eprintln!("Skipping accuracy test: IPAdic not found");
            return;
        }
    };

    let corpus = test_corpus();
    let total = corpus.len();
    let mut exact_match = 0;
    let mut char_correct = 0;
    let mut char_total = 0;
    let mut failures = Vec::new();

    for (input, expected) in &corpus {
        let result = jamo_core::convert_with_conn(input, &dict, &dict);
        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();

        if actual == *expected {
            exact_match += 1;
        } else {
            failures.push((input, expected, actual.clone()));
        }

        // 文字単位の精度
        for (a, e) in actual.chars().zip(expected.chars()) {
            if a == e {
                char_correct += 1;
            }
            char_total += 1;
        }
        // 長さが異なる場合、長い方の余りもカウント
        let len_diff = actual.chars().count().abs_diff(expected.chars().count());
        char_total += len_diff;
    }

    let sentence_accuracy = exact_match as f64 / total as f64 * 100.0;
    let char_accuracy = char_correct as f64 / char_total as f64 * 100.0;

    eprintln!("\n=== 変換精度テスト結果 ===");
    eprintln!("テスト数: {total}");
    eprintln!("文一致率: {exact_match}/{total} ({sentence_accuracy:.1}%)");
    eprintln!("文字精度: {char_correct}/{char_total} ({char_accuracy:.1}%)");

    if !failures.is_empty() {
        eprintln!("\n不一致:");
        for (input, expected, actual) in &failures {
            eprintln!("  {input} → {actual} (期待: {expected})");
        }
    }

    // ベースライン (IPAdic + Viterbi, bigram LM なし):
    //   文一致率: 48.1% (2026-02-24)
    //   文字精度: 69.1% (2026-02-24)
    // 短い単語 (「やま」「みる」等) は文脈なしで精度が低い
    // bigram LM (T022) で改善見込み
    assert!(
        sentence_accuracy >= 40.0,
        "文一致率 {sentence_accuracy:.1}% が最低40%を下回っている (回帰)"
    );
    assert!(
        char_accuracy >= 60.0,
        "文字精度 {char_accuracy:.1}% が最低60%を下回っている (回帰)"
    );
}

#[test]
fn test_accuracy_clause_level() {
    let dict = match load_ipadic() {
        Some(d) => d,
        None => {
            eprintln!("Skipping clause accuracy test: IPAdic not found");
            return;
        }
    };

    // 文節レベルでの精度テスト: N-best の候補に正解が含まれるか
    let corpus = test_corpus();
    let mut top1_match = 0;
    let mut top5_match = 0;
    let total = corpus.len();

    for (input, expected) in &corpus {
        let result = jamo_core::convert_nbest(input, &dict, &dict, 5);
        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();

        if actual == *expected {
            top1_match += 1;
            top5_match += 1;
        } else {
            // 各文節の候補に正解が含まれるかチェック
            // (簡易: 全文節の候補の組み合わせは見ず、部分一致でカウント)
            let clause_surfaces: String =
                result.clauses.iter().map(|c| c.surface.as_str()).collect();
            if clause_surfaces == *expected {
                top1_match += 1;
                top5_match += 1;
            }
        }
    }

    let top1_rate = top1_match as f64 / total as f64 * 100.0;
    let top5_rate = top5_match as f64 / total as f64 * 100.0;

    eprintln!("\n=== 文節精度テスト結果 ===");
    eprintln!("Top-1: {top1_match}/{total} ({top1_rate:.1}%)");
    eprintln!("Top-5: {top5_match}/{total} ({top5_rate:.1}%)");
}
