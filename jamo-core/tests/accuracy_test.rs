// T030 + T051: 変換精度テスト / 精度評価基盤
// IPAdic を使った変換精度のベースライン計測 + カテゴリ別評価

use std::path::Path;

/// テストコーパスのカテゴリ
#[derive(Debug, Clone, Copy, PartialEq)]
enum Category {
    NounPhrase,
    Adjective,
    VerbConjugation,
    Polite,
    Daily,
    Place,
    Time,
    Complex,
    ShortVerb,
    ShortNoun,
}

/// テストケース
struct TestCase {
    input: &'static str,
    expected: &'static str,
    category: Category,
}

fn test_corpus() -> Vec<TestCase> {
    vec![
        // 基本的な名詞+助詞
        TestCase {
            input: "わたしはがくせいです",
            expected: "私は学生です",
            category: Category::NounPhrase,
        },
        TestCase {
            input: "にほんごをべんきょうする",
            expected: "日本語を勉強する",
            category: Category::NounPhrase,
        },
        TestCase {
            input: "とうきょうにいきたい",
            expected: "東京に行きたい",
            category: Category::NounPhrase,
        },
        TestCase {
            input: "ほんをよむ",
            expected: "本を読む",
            category: Category::NounPhrase,
        },
        TestCase {
            input: "みずをのむ",
            expected: "水を飲む",
            category: Category::NounPhrase,
        },
        // 形容詞
        TestCase {
            input: "たかいやま",
            expected: "高い山",
            category: Category::Adjective,
        },
        TestCase {
            input: "おおきいいえ",
            expected: "大きい家",
            category: Category::Adjective,
        },
        TestCase {
            input: "あたらしいほん",
            expected: "新しい本",
            category: Category::Adjective,
        },
        // 動詞活用
        TestCase {
            input: "あるいていく",
            expected: "歩いていく",
            category: Category::VerbConjugation,
        },
        TestCase {
            input: "たべている",
            expected: "食べている",
            category: Category::VerbConjugation,
        },
        TestCase {
            input: "はしっている",
            expected: "走っている",
            category: Category::VerbConjugation,
        },
        // 丁寧語
        TestCase {
            input: "おはようございます",
            expected: "おはようございます",
            category: Category::Polite,
        },
        TestCase {
            input: "ありがとうございます",
            expected: "ありがとうございます",
            category: Category::Polite,
        },
        // 日常表現
        TestCase {
            input: "なまえはたなかです",
            expected: "名前は田中です",
            category: Category::Daily,
        },
        TestCase {
            input: "がっこうにいく",
            expected: "学校に行く",
            category: Category::Daily,
        },
        TestCase {
            input: "しごとをする",
            expected: "仕事をする",
            category: Category::Daily,
        },
        TestCase {
            input: "でんわをかける",
            expected: "電話をかける",
            category: Category::Daily,
        },
        TestCase {
            input: "てがみをかく",
            expected: "手紙を書く",
            category: Category::Daily,
        },
        // 場所
        TestCase {
            input: "おおさかにすんでいます",
            expected: "大阪に住んでいます",
            category: Category::Place,
        },
        TestCase {
            input: "えきまであるく",
            expected: "駅まで歩く",
            category: Category::Place,
        },
        // 時間
        TestCase {
            input: "あしたはやすみです",
            expected: "明日は休みです",
            category: Category::Time,
        },
        TestCase {
            input: "まいにちべんきょうする",
            expected: "毎日勉強する",
            category: Category::Time,
        },
        // やや複雑な文
        TestCase {
            input: "にほんのしゅとはとうきょうです",
            expected: "日本の首都は東京です",
            category: Category::Complex,
        },
        TestCase {
            input: "ともだちとえいがをみる",
            expected: "友達と映画を見る",
            category: Category::Complex,
        },
        TestCase {
            input: "かぞくでりょこうにいく",
            expected: "家族で旅行に行く",
            category: Category::Complex,
        },
        TestCase {
            input: "さんにんのがくせい",
            expected: "三人の学生",
            category: Category::Complex,
        },
        TestCase {
            input: "あめがふっているからかさをもつ",
            expected: "雨が降っているから傘を持つ",
            category: Category::Complex,
        },
        TestCase {
            input: "さむいのであたたかいものをたべる",
            expected: "寒いので暖かいものを食べる",
            category: Category::Complex,
        },
        // 短い動詞
        TestCase {
            input: "いく",
            expected: "行く",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "みる",
            expected: "見る",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "する",
            expected: "する",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "くる",
            expected: "来る",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "ある",
            expected: "ある",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "いる",
            expected: "いる",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "たべる",
            expected: "食べる",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "のむ",
            expected: "飲む",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "よむ",
            expected: "読む",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "かく",
            expected: "書く",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "はなす",
            expected: "話す",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "きく",
            expected: "聞く",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "おもう",
            expected: "思う",
            category: Category::ShortVerb,
        },
        TestCase {
            input: "しる",
            expected: "知る",
            category: Category::ShortVerb,
        },
        // 短い名詞
        TestCase {
            input: "やま",
            expected: "山",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "かわ",
            expected: "川",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "そら",
            expected: "空",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "うみ",
            expected: "海",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "はな",
            expected: "花",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "ひと",
            expected: "人",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "くるま",
            expected: "車",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "でんしゃ",
            expected: "電車",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "ひこうき",
            expected: "飛行機",
            category: Category::ShortNoun,
        },
        TestCase {
            input: "びょういん",
            expected: "病院",
            category: Category::ShortNoun,
        },
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

    // カテゴリ別集計
    let categories = [
        Category::NounPhrase,
        Category::Adjective,
        Category::VerbConjugation,
        Category::Polite,
        Category::Daily,
        Category::Place,
        Category::Time,
        Category::Complex,
        Category::ShortVerb,
        Category::ShortNoun,
    ];
    let mut cat_total: Vec<usize> = vec![0; categories.len()];
    let mut cat_match: Vec<usize> = vec![0; categories.len()];

    for tc in &corpus {
        let result = jamo_core::convert_with_conn(tc.input, &dict, &dict);
        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();

        let cat_idx = categories.iter().position(|c| *c == tc.category).unwrap();
        cat_total[cat_idx] += 1;

        if actual == tc.expected {
            exact_match += 1;
            cat_match[cat_idx] += 1;
        } else {
            failures.push((tc.input, tc.expected, actual.clone(), tc.category));
        }

        for (a, e) in actual.chars().zip(tc.expected.chars()) {
            if a == e {
                char_correct += 1;
            }
            char_total += 1;
        }
        let len_diff = actual.chars().count().abs_diff(tc.expected.chars().count());
        char_total += len_diff;
    }

    let sentence_accuracy = exact_match as f64 / total as f64 * 100.0;
    let char_accuracy = char_correct as f64 / char_total as f64 * 100.0;

    eprintln!("\n=== 変換精度テスト結果 ===");
    eprintln!("テスト数: {total}");
    eprintln!("文一致率: {exact_match}/{total} ({sentence_accuracy:.1}%)");
    eprintln!("文字精度: {char_correct}/{char_total} ({char_accuracy:.1}%)");

    eprintln!("\n--- カテゴリ別 ---");
    for (i, cat) in categories.iter().enumerate() {
        if cat_total[i] > 0 {
            let rate = cat_match[i] as f64 / cat_total[i] as f64 * 100.0;
            eprintln!("  {cat:?}: {}/{} ({rate:.0}%)", cat_match[i], cat_total[i]);
        }
    }

    if !failures.is_empty() {
        eprintln!("\n不一致:");
        for (input, expected, actual, cat) in &failures {
            eprintln!("  [{cat:?}] {input} → {actual} (期待: {expected})");
        }
    }

    // 回帰検出閾値
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

    let corpus = test_corpus();
    let mut top1_match = 0;
    let mut top5_match = 0;
    let total = corpus.len();

    for tc in &corpus {
        let result = jamo_core::convert_nbest(tc.input, &dict, &dict, 5);
        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();

        if actual == tc.expected {
            top1_match += 1;
            top5_match += 1;
        } else {
            let clause_surfaces: String =
                result.clauses.iter().map(|c| c.surface.as_str()).collect();
            if clause_surfaces == tc.expected {
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
