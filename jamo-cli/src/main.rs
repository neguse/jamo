use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use jamo_core::ipadic::IpaDict;

mod ime_server;
mod serve;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "convert" => cmd_convert(&args[2..]),
        "serve" => cmd_serve(&args[2..]),
        "dict" => cmd_dict(&args[2..]),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("Usage: jamo <command> [args]");
    eprintln!("Commands:");
    eprintln!("  convert [--dict DIR] <text>   かな漢字変換");
    eprintln!("  serve [--dict DIR] [--port N]  HTTPサーバー起動");
    eprintln!("  serve --ime [--dict DIR]       IME IPCサーバー起動");
    eprintln!(
        "  dict build [--dict DIR] -o <output>  辞書バイナリ構築 (DIR に id.def があれば mozc 形式)"
    );
    eprintln!("  dict merge --base <dir> --extra <path>... -o <output>  辞書マージ");
    eprintln!("  dict info <dict>               辞書バイナリのセクション別内訳");
}

/// 引数から --dict DIR を抽出。残りの引数も返す。
fn parse_dict_arg(args: &[String]) -> (Option<String>, Vec<String>) {
    let mut dict_path = None;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--dict" && i + 1 < args.len() {
            dict_path = Some(args[i + 1].clone());
            i += 2;
        } else {
            rest.push(args[i].clone());
            i += 1;
        }
    }
    (dict_path, rest)
}

/// 辞書ソースディレクトリをロードする (id.def があれば mozc、なければ IPAdic CSV)
fn load_source_dir(dir: &Path) -> IpaDict {
    if dir.join("id.def").exists() {
        eprintln!("Loading mozc dictionary from {}...", dir.display());
        jamo_core::mozc::load_mozc(dir).expect("failed to load mozc dictionary")
    } else {
        eprintln!("Loading IPAdic from {}...", dir.display());
        IpaDict::load(dir).expect("failed to load IPAdic")
    }
}

fn load_dict(dict_path: Option<&str>) -> Arc<IpaDict> {
    let start = Instant::now();

    // バイナリ辞書を優先
    if let Some(path) = dict_path {
        let bin_path = Path::new(path).with_extension("dict");
        if bin_path.exists() {
            eprintln!("Loading binary dict from {}...", bin_path.display());
            let dict = IpaDict::load_binary(&bin_path).expect("failed to load binary dict");
            eprintln!("Loaded in {:.2}s", start.elapsed().as_secs_f64());
            return Arc::new(dict);
        }
        let dict = Arc::new(load_source_dir(Path::new(path)));
        eprintln!("Loaded in {:.2}s", start.elapsed().as_secs_f64());
        return dict;
    }

    // デフォルトパスを試す (バイナリ優先、mozc 辞書があればそちらを優先)
    let bin_candidates = [
        "data/mozc.dict",
        "../data/mozc.dict",
        "data/ipadic.dict",
        "../data/ipadic.dict",
    ];
    for p in bin_candidates {
        if Path::new(p).exists() {
            eprintln!("Loading binary dict from {p}...");
            let dict = IpaDict::load_binary(Path::new(p)).expect("failed to load binary dict");
            eprintln!("Loaded in {:.2}s", start.elapsed().as_secs_f64());
            return Arc::new(dict);
        }
    }

    let candidates = ["data/mozc", "../data/mozc", "data/ipadic", "../data/ipadic"];
    for p in candidates {
        if Path::new(p).exists() {
            let dict = Arc::new(load_source_dir(Path::new(p)));
            eprintln!("Loaded in {:.2}s", start.elapsed().as_secs_f64());
            return dict;
        }
    }

    eprintln!("Error: no dictionary available");
    eprintln!("Hint: download IPAdic to data/ipadic/ or use --dict <path>");
    std::process::exit(1);
}

fn cmd_convert(args: &[String]) {
    let (dict_path, rest) = parse_dict_arg(args);

    if rest.is_empty() {
        eprintln!("Usage: jamo convert [--dict DIR] <text>");
        std::process::exit(1);
    }

    let dict = load_dict(dict_path.as_deref());
    let input = &rest[0];
    let result = jamo_core::convert_with_conn(input, dict.as_ref(), dict.as_ref());

    let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
    println!("{text}");
}

fn cmd_serve(args: &[String]) {
    let (dict_path, rest) = parse_dict_arg(args);

    // --ime モード: IME IPC サーバー
    if rest.iter().any(|a| a == "--ime") {
        let dict = load_dict(dict_path.as_deref());
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(ime_server::run(dict));
        return;
    }

    let mut port: u16 = 8080;
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--port" && i + 1 < rest.len() {
            port = rest[i + 1].parse().unwrap_or(8080);
            i += 2;
        } else {
            i += 1;
        }
    }

    let dict = load_dict(dict_path.as_deref());

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(serve::run(port, dict));
}

fn cmd_dict(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage:");
        eprintln!("  jamo dict build [--dict DIR] -o <output>");
        eprintln!("  jamo dict merge --base <dir> --extra <path>... -o <output>");
        std::process::exit(1);
    }

    match args[0].as_str() {
        "build" => cmd_dict_build(&args[1..]),
        "merge" => cmd_dict_merge(&args[1..]),
        "info" => cmd_dict_info(&args[1..]),
        _ => {
            eprintln!("Unknown dict subcommand: {}", args[0]);
            std::process::exit(1);
        }
    }
}

fn cmd_dict_info(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("Usage: jamo dict info <dict>");
        std::process::exit(1);
    };
    let data = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("Error: {path}: {e}");
        std::process::exit(1);
    });
    let s = jamo_core::dict_stats::dict_stats(&data).unwrap_or_else(|e| {
        eprintln!("Error: {path}: {e}");
        std::process::exit(1);
    });

    let total = s.total_bytes as f64;
    let row = |name: &str, bytes: usize, note: String| {
        println!(
            "{name:<16} {:8.1} MB  {:5.1}%  {note}",
            bytes as f64 / 1e6,
            bytes as f64 / total * 100.0
        );
    };
    println!("{path} (v{}, total {:.1} MB)", s.version, total / 1e6);
    row(
        "louds bitvec",
        s.louds_bytes,
        format!("{} bits", s.louds_bits),
    );
    row("labels", s.labels_bytes, format!("{} nodes", s.node_count));
    row(
        "terminal",
        s.terminal_bytes,
        format!(
            "{} nodes, sentinel {:.1}%",
            s.terminal_count,
            s.sentinel_count as f64 / s.terminal_count.max(1) as f64 * 100.0
        ),
    );
    row(
        "entries",
        s.entries_bytes,
        format!("{} entries", s.entry_count),
    );
    row(
        "  surface",
        s.surface_bytes,
        format!(
            "uniq {} ({:.1} MB)",
            s.surface_uniq_count,
            s.surface_uniq_bytes as f64 / 1e6
        ),
    );
    row(
        "  fixed 9B",
        s.entry_fixed_bytes,
        "len + left/right/cost/pos_type".to_string(),
    );
    row(
        "conn matrix",
        s.matrix_bytes,
        format!("num_pos {}", s.num_pos),
    );
    println!(
        "stats: vocab {}, readings {}",
        s.vocab_size, s.reading_count
    );
}

fn cmd_dict_merge(args: &[String]) {
    let mut base_dir = None;
    let mut extras = Vec::new();
    let mut output = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--base" if i + 1 < args.len() => {
                base_dir = Some(args[i + 1].clone());
                i += 2;
            }
            "--extra" if i + 1 < args.len() => {
                extras.push(args[i + 1].clone());
                i += 2;
            }
            "-o" if i + 1 < args.len() => {
                output = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    let base_dir = base_dir.unwrap_or_else(|| {
        eprintln!("Usage: jamo dict merge --base <dir> --extra <path>... -o <output>");
        std::process::exit(1);
    });
    let output = output.unwrap_or_else(|| {
        eprintln!("Usage: jamo dict merge --base <dir> --extra <path>... -o <output>");
        std::process::exit(1);
    });
    if extras.is_empty() {
        eprintln!("Error: at least one --extra path required");
        std::process::exit(1);
    }

    let start = Instant::now();

    let extra_paths: Vec<&Path> = extras.iter().map(|s| Path::new(s.as_str())).collect();
    eprintln!(
        "Merging base={} + {} extra source(s)...",
        base_dir,
        extras.len()
    );
    let dict = IpaDict::load_merge(Path::new(&base_dir), &extra_paths)
        .expect("failed to merge dictionaries");
    let load_time = start.elapsed();

    eprintln!(
        "Merged {} vocab, {} readings in {:.2}s",
        dict.vocab_size(),
        dict.reading_count(),
        load_time.as_secs_f64()
    );

    let save_start = Instant::now();
    dict.save_binary(Path::new(&output))
        .expect("failed to save binary dict");
    let save_time = save_start.elapsed();

    let file_size = std::fs::metadata(&output).unwrap().len();
    eprintln!(
        "Saved to {output} ({:.1} MB) in {:.2}s",
        file_size as f64 / 1_048_576.0,
        save_time.as_secs_f64()
    );
}

fn cmd_dict_build(args: &[String]) {
    let (dict_path, rest) = parse_dict_arg(args);

    let mut output = None;
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "-o" && i + 1 < rest.len() {
            output = Some(rest[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }

    let output = output.unwrap_or_else(|| {
        eprintln!("Usage: jamo dict build [--dict DIR] -o <output>");
        std::process::exit(1);
    });

    let start = Instant::now();

    // ソースロード (mozc / IPAdic 自動判別)
    let dict_dir = dict_path.as_deref().unwrap_or("data/ipadic");
    let dict = load_source_dir(Path::new(dict_dir));
    let load_time = start.elapsed();

    eprintln!(
        "Loaded {} vocab, {} readings in {:.2}s",
        dict.vocab_size(),
        dict.reading_count(),
        load_time.as_secs_f64()
    );

    // バイナリ書き出し
    let save_start = Instant::now();
    dict.save_binary(Path::new(&output))
        .expect("failed to save binary dict");
    let save_time = save_start.elapsed();

    let file_size = std::fs::metadata(&output).unwrap().len();
    eprintln!(
        "Saved to {output} ({:.1} MB) in {:.2}s",
        file_size as f64 / 1_048_576.0,
        save_time.as_secs_f64()
    );
}
