//! `jamo bench <corpus.tsv>`: コーパス一括変換の精度・レイテンシ・メモリ計測
//!
//! scripts/bench-engines.py が mozc_server と同じコーパスで比較するために使う。
//! 1 行ごとに `input\texpected\ttop1\ttop5\tlatency_us` を出力し、最後に
//! `#summary {json}` を出す。メモリは /proc/self/status の VmHWM (peak RSS)。

use std::path::Path;
use std::time::Instant;

use jamo_core::bindict::BinDict;
use jamo_core::viterbi::Clause;

pub struct BenchArgs<'a> {
    pub corpus: &'a Path,
    pub repeat: usize,
}

/// 期待文を、各文節の上位 top_n 候補から 1 つずつ選んで組み立てられるか
fn reachable(clauses: &[Clause], expected: &str, top_n: usize) -> bool {
    let Some((first, rest)) = clauses.split_first() else {
        return expected.is_empty();
    };
    first.candidates.iter().take(top_n).any(|c| {
        expected
            .strip_prefix(c.as_str())
            .is_some_and(|tail| reachable(rest, tail, top_n))
    })
}

/// /proc/self/status の指定フィールド (kB)
fn proc_status_kb(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|l| l.starts_with(field))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// コーパスを変換して集計する。`load_us` は辞書ロードに掛かった時間
pub fn run(
    dict: &BinDict,
    dict_bytes: u64,
    load_us: f64,
    args: &BenchArgs<'_>,
) -> Result<(), String> {
    let corpus = std::fs::read_to_string(args.corpus)
        .map_err(|e| format!("read {}: {e}", args.corpus.display()))?;
    let cases: Vec<(&str, &str)> = corpus
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() == 3 {
                Ok((f[1], f[2]))
            } else {
                Err(format!("corpus: 3 columns expected: {l}"))
            }
        })
        .collect::<Result<_, _>>()?;

    let repeat = args.repeat.max(1);
    let mut top1 = 0;
    let mut top5 = 0;
    let mut latencies = Vec::with_capacity(cases.len());
    for (input, expected) in &cases {
        // 計測 1 回目はウォームアップ (ページイン込み) として捨てる
        let mut result = jamo_core::convert_nbest(input, dict, dict, 5);
        let start = Instant::now();
        for _ in 0..repeat {
            result = jamo_core::convert_nbest(input, dict, dict, 5);
        }
        let us = start.elapsed().as_secs_f64() * 1e6 / repeat as f64;
        latencies.push(us);

        let actual: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
        let hit1 = actual == *expected;
        let hit5 = reachable(&result.clauses, expected, 5);
        top1 += hit1 as usize;
        top5 += hit5 as usize;
        println!("{input}\t{expected}\t{actual}\t{}\t{us:.1}", hit5 as u8);
    }

    let mut sorted = latencies.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = latencies.iter().sum::<f64>() / latencies.len().max(1) as f64;
    let summary = serde_json::json!({
        "total": cases.len(),
        "top1": top1,
        "top5": top5,
        "mean_us": mean,
        "p50_us": percentile(&sorted, 0.5),
        "p95_us": percentile(&sorted, 0.95),
        "load_us": load_us,
        "dict_bytes": dict_bytes,
        "vm_hwm_kb": proc_status_kb("VmHWM:"),
        "vm_rss_kb": proc_status_kb("VmRSS:"),
    });
    println!("#summary {summary}");
    Ok(())
}
