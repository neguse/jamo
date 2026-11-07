// 逆変換 (表層 → 読み) を mozc 辞書で確かめる (辞書がなければ skip)

use std::path::Path;

use jamo_core::bindict::BinDict;

fn load_mozc_dict() -> Option<BinDict> {
    for p in ["../data/mozc.dict", "data/mozc.dict"] {
        let path = Path::new(p);
        if path.exists() {
            return BinDict::open(path).ok();
        }
    }
    for p in ["../data/mozc", "data/mozc"] {
        let path = Path::new(p);
        if path.exists() {
            return jamo_core::mozc::load_mozc(path).ok()?.into_bindict().ok();
        }
    }
    None
}

fn reading(dict: &BinDict, text: &str) -> String {
    jamo_core::reverse::reading(text, dict, dict)
        .iter()
        .map(|w| w.reading.as_str())
        .collect()
}

#[test]
fn test_reverse_mozc() {
    let Some(dict) = load_mozc_dict() else {
        eprintln!("Skipping reverse test: data/mozc not found");
        return;
    };
    let cases = [
        ("東京に行きたい", "とうきょうにいきたい"),
        ("私は学生です", "わたしはがくせいです"),
        ("日本語を勉強する", "にほんごをべんきょうする"),
        ("今日は良い天気です", "きょうはよいてんきです"),
        // 送りがなと活用
        ("触らない", "さわらない"),
        ("通す", "とおす"),
        ("雨が降る", "あめがふる"),
        ("日が暮れる", "ひがくれる"),
        // 熟字訓
        ("大人", "おとな"),
        ("一人で", "ひとりで"),
        ("明日", "あした"),
        // 同じ字でも前後の語で読みが決まる
        ("待っている間に", "まっているあいだに"),
        ("間に合う", "まにあう"),
        // カタカナはひらがなにする
        ("ボールを投げる", "ぼーるをなげる"),
    ];
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|&(text, want)| {
            let got = reading(&dict, text);
            (got != want).then(|| format!("{text}: want {want}, got {got}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn test_reverse_words_cover_input() {
    let Some(dict) = load_mozc_dict() else {
        eprintln!("Skipping reverse test: data/mozc not found");
        return;
    };
    // 辞書に無い字も落とさず、語の表層をつなぐと入力に戻る
    let text = "猫と★x";
    let words = jamo_core::reverse::reading(text, &dict, &dict);
    let surface: String = words.iter().map(|w| w.surface.as_str()).collect();
    assert_eq!(surface, text);
    let star = words.iter().find(|w| w.surface == "★").expect("★ の語");
    assert_eq!(star.reading, "★");
}
