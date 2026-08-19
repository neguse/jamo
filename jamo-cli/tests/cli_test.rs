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
fn test_cli_dict_build_and_info() {
    let tmp_dir = std::env::temp_dir().join("jamo_cli_dict_info_test");
    let _ = std::fs::create_dir_all(&tmp_dir);
    std::fs::write(
        tmp_dir.join("mini.csv"),
        "今日,0,0,3000,名詞,一般,*,*,*,*,今日,キョウ,キョウ\n",
    )
    .unwrap();
    std::fs::write(tmp_dir.join("matrix.def"), "1 1\n0 0 0\n").unwrap();
    let dict_bin = tmp_dir.join("mini.dict");

    let build = Command::new(env!("CARGO_BIN_EXE_jamo-cli"))
        .args(["dict", "build", "--dict"])
        .arg(&tmp_dir)
        .arg("-o")
        .arg(&dict_bin)
        .output()
        .expect("failed to run dict build");
    assert!(build.status.success(), "dict build failed: {build:?}");

    let info = Command::new(env!("CARGO_BIN_EXE_jamo-cli"))
        .args(["dict", "info"])
        .arg(&dict_bin)
        .output()
        .expect("failed to run dict info");
    assert!(info.status.success(), "dict info failed: {info:?}");
    let stdout = String::from_utf8(info.stdout).expect("invalid utf8");
    assert!(stdout.contains("total"), "should print total: {stdout}");
    assert!(
        stdout.contains("1 entries"),
        "should count entries: {stdout}"
    );
    assert!(
        stdout.contains("num_pos 1"),
        "should show num_pos: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
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
