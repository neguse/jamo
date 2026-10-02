// T030 + T051: 変換精度テスト / 精度評価基盤
// IPAdic を使った変換精度のベースライン計測 + カテゴリ別評価

use std::path::Path;

use jamo_core::bindict::BinDict;

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
    Katakana,
}

/// テストケース (tests/corpus.tsv の 1 行)
struct TestCase {
    input: &'static str,
    expected: &'static str,
    category: Category,
}

/// 精度評価コーパス。scripts/bench-engines.py (mozc との比較) と同じファイルを読む
const CORPUS_TSV: &str = include_str!("corpus.tsv");

fn parse_category(name: &str) -> Category {
    match name {
        "NounPhrase" => Category::NounPhrase,
        "Adjective" => Category::Adjective,
        "VerbConjugation" => Category::VerbConjugation,
        "Polite" => Category::Polite,
        "Daily" => Category::Daily,
        "Place" => Category::Place,
        "Time" => Category::Time,
        "Complex" => Category::Complex,
        "ShortVerb" => Category::ShortVerb,
        "ShortNoun" => Category::ShortNoun,
        "Katakana" => Category::Katakana,
        other => panic!("unknown category in corpus.tsv: {other}"),
    }
}

fn test_corpus() -> Vec<TestCase> {
    CORPUS_TSV
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            assert_eq!(f.len(), 3, "corpus.tsv: 3 columns expected: {l}");
            TestCase {
                input: f[1],
                expected: f[2],
                category: parse_category(f[0]),
            }
        })
        .collect()
}

fn load_ipadic() -> Option<BinDict> {
    for p in ["../data/ipadic.dict", "data/ipadic.dict"] {
        let path = Path::new(p);
        if path.exists() {
            return BinDict::open(path).ok();
        }
    }
    for p in ["../data/ipadic", "data/ipadic"] {
        let path = Path::new(p);
        if path.exists() {
            return jamo_core::ipadic::IpaDict::load(path)
                .ok()?
                .into_bindict()
                .ok();
        }
    }
    None
}

fn load_mozc_dict() -> Option<BinDict> {
    for p in ["../data/mozc.dict", "data/mozc.dict"] {
        let path = Path::new(p);
        if path.exists() {
            return BinDict::open(path).ok();
        }
    }
    for p in ["../data/mozc", "data/mozc"] {
        let path = Path::new(p);
        if path.join("id.def").exists() {
            return jamo_core::mozc::load_mozc(path).ok()?.into_bindict().ok();
        }
    }
    None
}

/// mozc 辞書での精度 (辞書がなければ skip)
#[test]
fn test_accuracy_mozc() {
    let dict = match load_mozc_dict() {
        Some(d) => d,
        None => {
            eprintln!("Skipping mozc accuracy test: data/mozc not found");
            return;
        }
    };

    let corpus = test_corpus();
    let total = corpus.len();
    let mut top1 = 0;
    let mut top5 = 0;
    let mut failures = Vec::new();

    for tc in &corpus {
        let result = jamo_core::convert_nbest(tc.input, &dict, &dict, 5);
        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
        if actual == tc.expected {
            top1 += 1;
        } else {
            failures.push((tc.input, tc.expected, actual));
        }
        if reachable_with_candidates(&result.clauses, tc.expected, 5) {
            top5 += 1;
        }
    }

    let top1_rate = top1 as f64 / total as f64 * 100.0;
    let top5_rate = top5 as f64 / total as f64 * 100.0;
    eprintln!("\n=== mozc 辞書精度 ===");
    eprintln!("Top-1: {top1}/{total} ({top1_rate:.1}%)");
    eprintln!("Top-5: {top5}/{total} ({top5_rate:.1}%)");
    for (input, expected, actual) in &failures {
        eprintln!("  {input} → {actual} (期待: {expected})");
    }

    assert!(
        top1_rate >= 80.0,
        "mozc Top-1 {top1_rate:.1}% が最低80%を下回っている (回帰)"
    );
    assert!(
        top5_rate >= 90.0,
        "mozc Top-5 {top5_rate:.1}% が最低90%を下回っている (回帰)"
    );
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
        Category::Katakana,
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

/// 期待文を、各文節の上位 top_n 候補から1つずつ選んで組み立てられるか
/// (= ユーザーが候補選択だけで正解にたどり着けるか)
fn reachable_with_candidates(
    clauses: &[jamo_core::viterbi::Clause],
    expected: &str,
    top_n: usize,
) -> bool {
    let Some((first, rest)) = clauses.split_first() else {
        return expected.is_empty();
    };
    first.candidates.iter().take(top_n).any(|c| {
        expected
            .strip_prefix(c.as_str())
            .is_some_and(|tail| reachable_with_candidates(rest, tail, top_n))
    })
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
    let mut unreachable = Vec::new();

    for tc in &corpus {
        let result = jamo_core::convert_nbest(tc.input, &dict, &dict, 5);
        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();

        if actual == tc.expected {
            top1_match += 1;
        }
        if reachable_with_candidates(&result.clauses, tc.expected, 5) {
            top5_match += 1;
        } else {
            unreachable.push(tc.input);
        }
    }

    let top1_rate = top1_match as f64 / total as f64 * 100.0;
    let top5_rate = top5_match as f64 / total as f64 * 100.0;

    eprintln!("\n=== 文節精度テスト結果 ===");
    eprintln!("Top-1: {top1_match}/{total} ({top1_rate:.1}%)");
    eprintln!("Top-5 (候補選択で到達可能): {top5_match}/{total} ({top5_rate:.1}%)");
    if !unreachable.is_empty() {
        eprintln!("候補5位以内で到達不能: {unreachable:?}");
    }

    // 回帰検出閾値
    assert!(
        top5_rate >= 60.0,
        "Top-5 到達率 {top5_rate:.1}% が最低60%を下回っている (回帰)"
    );
}
