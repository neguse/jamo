use criterion::{Criterion, black_box, criterion_group, criterion_main};
use jamo_core::bindict::BinDict;
use std::path::Path;

fn load_dict() -> Option<BinDict> {
    let paths = ["data/ipadic.dict", "../data/ipadic.dict"];
    for p in &paths {
        if Path::new(p).exists() {
            return BinDict::open(Path::new(p)).ok();
        }
    }
    None
}

fn bench_convert(c: &mut Criterion) {
    let dict = match load_dict() {
        Some(d) => d,
        None => {
            eprintln!("SKIP: ipadic.dict not found");
            return;
        }
    };

    let inputs = [
        ("short", "きょう"),
        ("medium", "きょうはいいてんきですね"),
        ("long", "わたしはにほんごをべんきょうしています"),
        ("complex", "とうきょうとっきょきょかきょくきょくちょう"),
    ];

    let mut group = c.benchmark_group("convert");
    for (name, input) in &inputs {
        group.bench_function(*name, |b| {
            b.iter(|| jamo_core::convert_nbest(black_box(input), &dict, &dict, 5))
        });
    }
    group.finish();
}

fn bench_dict_load(c: &mut Criterion) {
    let paths = ["data/ipadic.dict", "../data/ipadic.dict"];
    let path = paths.iter().find(|p| Path::new(p).exists());
    let path = match path {
        Some(p) => *p,
        None => {
            eprintln!("SKIP: ipadic.dict not found");
            return;
        }
    };

    c.bench_function("dict_load", |b| {
        b.iter(|| BinDict::open(Path::new(black_box(path))).unwrap())
    });
}

fn bench_prefix_search(c: &mut Criterion) {
    let dict = match load_dict() {
        Some(d) => d,
        None => {
            eprintln!("SKIP: ipadic.dict not found");
            return;
        }
    };

    let prefixes = [("1char", "き"), ("2char", "きょ"), ("3char", "きょう")];

    let mut group = c.benchmark_group("prefix_search");
    for (name, prefix) in &prefixes {
        group.bench_function(*name, |b| {
            b.iter(|| dict.prefix_search(black_box(prefix), 10))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_convert, bench_dict_load, bench_prefix_search);
criterion_main!(benches);
