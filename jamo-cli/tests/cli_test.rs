use std::process::Command;

fn jamo_convert(input: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_jamo-cli"))
        .args(["convert", input])
        .output()
        .expect("failed to execute jamo-cli");

    assert!(output.status.success(), "jamo-cli failed: {:?}", output);
    String::from_utf8(output.stdout)
        .expect("invalid utf8")
        .trim()
        .to_string()
}

#[test]
fn test_cli_convert_kyouha() {
    assert_eq!(jamo_convert("きょうは"), "今日は");
}

#[test]
fn test_cli_convert_desu() {
    assert_eq!(jamo_convert("です"), "です");
}

#[test]
fn test_cli_convert_covers_input() {
    let result = jamo_convert("きょうはいいてんきですね");
    assert!(!result.is_empty());
}
