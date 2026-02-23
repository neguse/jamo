use std::process::Command;

fn jamo_convert(input: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_jamo-cli"))
        .args(["convert", "--dict", "data/ipadic", input])
        .output()
        .expect("failed to execute jamo-cli");

    assert!(output.status.success(), "jamo-cli failed: {:?}", output);
    String::from_utf8(output.stdout)
        .expect("invalid utf8")
        .trim()
        .to_string()
}

fn ipadic_available() -> bool {
    std::path::Path::new("data/ipadic").exists()
}

#[test]
fn test_cli_convert_watashi() {
    if !ipadic_available() {
        return;
    }
    assert_eq!(jamo_convert("わたしはがくせいです"), "私は学生です");
}

#[test]
fn test_cli_convert_tokyo() {
    if !ipadic_available() {
        return;
    }
    assert_eq!(jamo_convert("とうきょうにいきたい"), "東京に行きたい");
}

#[test]
fn test_cli_convert_nihongo() {
    if !ipadic_available() {
        return;
    }
    assert_eq!(jamo_convert("にほんごをべんきょうする"), "日本語を勉強する");
}

#[test]
fn test_cli_convert_covers_full_input() {
    if !ipadic_available() {
        return;
    }
    let result = jamo_convert("きょうはいいてんきですね");
    assert!(!result.is_empty());
    // 接続コスト付きで天気が出る
    assert!(result.contains("天気"), "天気が含まれるはず: {result}");
}
