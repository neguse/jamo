use std::env;

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
    eprintln!("  convert <text>     かな漢字変換");
    eprintln!("  serve [--port N]   HTTPサーバー起動");
}

fn cmd_convert(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: jamo convert <text>");
        std::process::exit(1);
    }

    let input = &args[0];
    let dict = jamo_core::dict::HashMapDict::test_dict();
    let result = jamo_core::convert(input, &dict);

    let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
    println!("{text}");
}

fn cmd_serve(args: &[String]) {
    let mut port: u16 = 8080;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            port = args[i + 1].parse().unwrap_or(8080);
            i += 2;
        } else {
            i += 1;
        }
    }

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(serve::run(port));
}
