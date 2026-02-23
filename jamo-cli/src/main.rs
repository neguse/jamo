use std::env;
use std::path::Path;
use std::sync::Arc;

use jamo_core::ipadic::IpaDict;

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

fn load_dict(dict_path: Option<&str>) -> Arc<IpaDict> {
    if let Some(path) = dict_path {
        eprintln!("Loading IPAdic from {path}...");
        Arc::new(IpaDict::load(Path::new(path)).expect("failed to load IPAdic"))
    } else {
        // デフォルトパスを試す
        let candidates = ["data/ipadic", "../data/ipadic"];
        for p in candidates {
            if Path::new(p).exists() {
                eprintln!("Loading IPAdic from {p}...");
                return Arc::new(IpaDict::load(Path::new(p)).expect("failed to load IPAdic"));
            }
        }
        // IPAdic が見つからない場合はテスト辞書にフォールバック
        eprintln!("IPAdic not found, using test dictionary");
        eprintln!("Hint: download IPAdic to data/ipadic/ or use --dict <path>");
        // テスト辞書で fallback はできない (型が違う) のでエラー
        eprintln!("Error: no dictionary available");
        std::process::exit(1);
    }
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
