use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: jamo <command> [args]");
        eprintln!("Commands:");
        eprintln!("  convert <text>  かな漢字変換");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "convert" => cmd_convert(&args[2..]),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
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
    println!("{}", text);
}
