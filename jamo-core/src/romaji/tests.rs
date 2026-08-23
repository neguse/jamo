use super::*;

/// ヘルパー: 文字列全体を feed して結果を収集
fn convert_all(input: &str) -> String {
    let mut conv = RomajiConverter::new();
    let mut result = String::new();
    for ch in input.chars() {
        if let RomajiResult::Emit(s) = conv.feed(ch) {
            result.push_str(&s);
        }
    }
    result.push_str(&conv.flush());
    result
}

#[test]
fn test_basic_vowels() {
    assert_eq!(convert_all("aiueo"), "あいうえお");
}

#[test]
fn test_ka_row() {
    assert_eq!(convert_all("kakikukeko"), "かきくけこ");
}

#[test]
fn test_sa_row() {
    assert_eq!(convert_all("sasisuseso"), "さしすせそ");
}

#[test]
fn test_ta_row() {
    assert_eq!(convert_all("tatituteto"), "たちつてと");
}

#[test]
fn test_na_row() {
    assert_eq!(convert_all("naninuneno"), "なにぬねの");
}

#[test]
fn test_ha_row() {
    assert_eq!(convert_all("hahihuheho"), "はひふへほ");
}

#[test]
fn test_ma_row() {
    assert_eq!(convert_all("mamimumemo"), "まみむめも");
}

#[test]
fn test_ya_row() {
    assert_eq!(convert_all("yayuyo"), "やゆよ");
}

#[test]
fn test_ra_row() {
    assert_eq!(convert_all("rarirurero"), "らりるれろ");
}

#[test]
fn test_wa_wo() {
    assert_eq!(convert_all("wawo"), "わを");
}

#[test]
fn test_wi_we() {
    // MS-IME 準拠: wi/we は現代表記、旧仮名は wyi/wye
    assert_eq!(convert_all("wiwe"), "うぃうぇ");
    assert_eq!(convert_all("wyiwye"), "ゐゑ");
}

#[test]
fn test_dakuon() {
    assert_eq!(convert_all("gagigugego"), "がぎぐげご");
    assert_eq!(convert_all("zazizuzezo"), "ざじずぜぞ");
    assert_eq!(convert_all("dadidudedo"), "だぢづでど");
    assert_eq!(convert_all("babibubebo"), "ばびぶべぼ");
}

#[test]
fn test_handakuon() {
    assert_eq!(convert_all("papipupepo"), "ぱぴぷぺぽ");
}

#[test]
fn test_special_consonants() {
    assert_eq!(convert_all("shi"), "し");
    assert_eq!(convert_all("chi"), "ち");
    assert_eq!(convert_all("tsu"), "つ");
    assert_eq!(convert_all("fu"), "ふ");
}

#[test]
fn test_youon() {
    assert_eq!(convert_all("kya"), "きゃ");
    assert_eq!(convert_all("sha"), "しゃ");
    assert_eq!(convert_all("cha"), "ちゃ");
    assert_eq!(convert_all("nya"), "にゃ");
    assert_eq!(convert_all("hya"), "ひゃ");
    assert_eq!(convert_all("mya"), "みゃ");
    assert_eq!(convert_all("rya"), "りゃ");
}

#[test]
fn test_sokuon() {
    assert_eq!(convert_all("kitte"), "きって");
    assert_eq!(convert_all("kekka"), "けっか");
    assert_eq!(convert_all("zasshi"), "ざっし");
    assert_eq!(convert_all("motto"), "もっと");
    assert_eq!(convert_all("happyou"), "はっぴょう");
}

#[test]
fn test_nn() {
    assert_eq!(convert_all("nn"), "ん");
    // MS-IME方式: "nn"は両方のnを消費して"ん"を確定
    // "nna" = nn(ん) + a(あ)
    assert_eq!(convert_all("nna"), "んあ");
}

#[test]
fn test_n_before_consonant() {
    assert_eq!(convert_all("shimbun"), "しんぶん");
    assert_eq!(convert_all("kangen"), "かんげん");
    assert_eq!(convert_all("senpai"), "せんぱい");
}

#[test]
fn test_n_before_vowel() {
    assert_eq!(convert_all("na"), "な");
    assert_eq!(convert_all("ni"), "に");
    assert_eq!(convert_all("nai"), "ない");
}

#[test]
fn test_kyou() {
    assert_eq!(convert_all("kyou"), "きょう");
}

#[test]
fn test_konnichiha() {
    // MS-IME方式: nn は両方消費して ん
    // "konnichiha" = ko(こ) + nn(ん) + i(い) + chi(ち) + ha(は)
    assert_eq!(convert_all("konnichiha"), "こんいちは");
    // こんにちは を打つには konnnichiha (3つのn)
    assert_eq!(convert_all("konnnichiha"), "こんにちは");
}

#[test]
fn test_toukyou() {
    assert_eq!(convert_all("toukyou"), "とうきょう");
}

#[test]
fn test_small_kana() {
    assert_eq!(convert_all("xa"), "ぁ");
    assert_eq!(convert_all("xi"), "ぃ");
    assert_eq!(convert_all("xtu"), "っ");
    assert_eq!(convert_all("la"), "ぁ");
    assert_eq!(convert_all("li"), "ぃ");
    assert_eq!(convert_all("ltu"), "っ");
}

#[test]
fn test_punctuation() {
    assert_eq!(convert_all("."), "。");
    assert_eq!(convert_all(","), "、");
    assert_eq!(convert_all("/"), "・");
    assert_eq!(convert_all("["), "「");
    assert_eq!(convert_all("]"), "」");
    assert_eq!(convert_all("-"), "ー");
}

#[test]
fn test_complex_sentence() {
    assert_eq!(convert_all("watashihanegusodesu"), "わたしはねぐそです");
}

#[test]
fn test_backspace_pending() {
    let mut conv = RomajiConverter::new();
    conv.feed('k');
    assert_eq!(conv.pending(), "k");
    assert_eq!(conv.backspace(), BackspaceResult::DeletedPending);
    assert_eq!(conv.pending(), "");
}

#[test]
fn test_backspace_pending_multi() {
    let mut conv = RomajiConverter::new();
    conv.feed('k');
    conv.feed('y');
    assert_eq!(conv.pending(), "ky");
    assert_eq!(conv.backspace(), BackspaceResult::DeletedPending);
    assert_eq!(conv.pending(), "k");
}

#[test]
fn test_backspace_empty() {
    let mut conv = RomajiConverter::new();
    assert_eq!(conv.backspace(), BackspaceResult::DeleteHiragana);
}

#[test]
fn test_flush_n() {
    let mut conv = RomajiConverter::new();
    conv.feed('n');
    assert_eq!(conv.pending(), "n");
    assert_eq!(conv.flush(), "ん");
    assert_eq!(conv.pending(), "");
}

#[test]
fn test_flush_empty() {
    let mut conv = RomajiConverter::new();
    assert_eq!(conv.flush(), "");
}

#[test]
fn test_reset() {
    let mut conv = RomajiConverter::new();
    conv.feed('k');
    conv.feed('y');
    conv.reset();
    assert_eq!(conv.pending(), "");
}

#[test]
fn test_ja_ji_ju() {
    assert_eq!(convert_all("ja"), "じゃ");
    assert_eq!(convert_all("ji"), "じ");
    assert_eq!(convert_all("ju"), "じゅ");
}

#[test]
fn test_long_vowel() {
    assert_eq!(convert_all("oniisan"), "おにいさん");
    assert_eq!(convert_all("okaasan"), "おかあさん");
}

#[test]
fn test_fa_fi_fe_fo() {
    assert_eq!(convert_all("fa"), "ふぁ");
    assert_eq!(convert_all("fi"), "ふぃ");
    assert_eq!(convert_all("fe"), "ふぇ");
    assert_eq!(convert_all("fo"), "ふぉ");
}

#[test]
fn test_pending_state() {
    let mut conv = RomajiConverter::new();
    assert_eq!(conv.feed('k'), RomajiResult::Pending);
    assert_eq!(conv.pending(), "k");
    assert_eq!(conv.feed('a'), RomajiResult::Emit("か".to_string()));
    assert_eq!(conv.pending(), "");
}

#[test]
fn test_sokuon_state() {
    let mut conv = RomajiConverter::new();
    assert_eq!(conv.feed('k'), RomajiResult::Pending);
    assert_eq!(conv.feed('k'), RomajiResult::Emit("っ".to_string()));
    assert_eq!(conv.pending(), "k");
    assert_eq!(conv.feed('a'), RomajiResult::Emit("か".to_string()));
}

// ---- T160: 取りこぼし修正 ----

#[test]
fn test_punctuation_after_pending_n() {
    assert_eq!(convert_all("hon."), "ほん。");
    assert_eq!(convert_all("kon,"), "こん、");
    assert_eq!(convert_all("hon!"), "ほん！");
}

#[test]
fn test_punctuation_after_pending_consonant_keeps_letter() {
    // 子音が未完成のまま句読点 → 子音は英字のまま残す (黙って消さない)
    assert_eq!(convert_all("k."), "k。");
}

#[test]
fn test_flush_keeps_pending_consonant() {
    let mut conv = RomajiConverter::new();
    for ch in "kyouk".chars() {
        conv.feed(ch);
    }
    assert_eq!(conv.pending(), "k");
    assert_eq!(conv.flush(), "k");
    assert_eq!(conv.pending(), "");
}

#[test]
fn test_flush_converts_n() {
    let mut conv = RomajiConverter::new();
    conv.feed('n');
    assert_eq!(conv.flush(), "ん");
}

#[test]
fn test_xtsu_ltsu() {
    assert_eq!(convert_all("xtsu"), "っ");
    assert_eq!(convert_all("ltsu"), "っ");
    assert_eq!(convert_all("kixtsuto"), "きっと");
}

#[test]
fn test_n_apostrophe() {
    // n' は ん を確定してアポストロフィを消費する (kan'i → かんい)
    assert_eq!(convert_all("n'"), "ん");
    assert_eq!(convert_all("kan'i"), "かんい");
}
