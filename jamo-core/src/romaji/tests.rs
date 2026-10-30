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
    // nn + 母音/y は ん + な行
    assert_eq!(convert_all("onna"), "おんな");
    assert_eq!(convert_all("kannin"), "かんにん");
    assert_eq!(convert_all("shinnyuu"), "しんにゅう");
    // n を 3 つ打っても同じ
    assert_eq!(convert_all("onnna"), "おんな");
    // nn + 子音は ん + 子音
    assert_eq!(convert_all("sannka"), "さんか");
    // ん + 母音/y は n' で打つ
    assert_eq!(convert_all("kan'i"), "かんい");
    assert_eq!(convert_all("hon'ya"), "ほんや");
    assert_eq!(convert_all("nnn"), "んん");
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
    assert_eq!(convert_all("konnichiha"), "こんにちは");
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

// ---- T162: かな→ローマ字 (F10 用の生ローマ字復元) ----

#[test]
fn test_kana_to_romaji_basic() {
    assert_eq!(kana_to_romaji("じゃも"), "jamo");
    assert_eq!(kana_to_romaji("きょう"), "kyou");
    assert_eq!(kana_to_romaji("しち"), "shichi");
    assert_eq!(kana_to_romaji("つ"), "tsu");
    assert_eq!(kana_to_romaji("ふぁいる"), "fairu");
}

#[test]
fn test_kana_to_romaji_sokuon_and_n() {
    assert_eq!(kana_to_romaji("きっと"), "kitto");
    assert_eq!(kana_to_romaji("あっ"), "axtu");
    assert_eq!(kana_to_romaji("かんい"), "kan'i");
    assert_eq!(kana_to_romaji("ほんや"), "hon'ya");
    assert_eq!(kana_to_romaji("かんな"), "kanna");
    for kana in ["かんい", "ほんや", "かんな", "んん", "んんな", "しんにゅう"] {
        assert_eq!(convert_all(&kana_to_romaji(kana)), kana);
    }
    assert_eq!(kana_to_romaji("ほん"), "hon");
    assert_eq!(kana_to_romaji("ほんき"), "honki");
}

#[test]
fn test_kana_to_romaji_keeps_other_chars() {
    // 英字 (英字モードの生文字)・句読点・ー・数字
    assert_eq!(kana_to_romaji("るst"), "rust");
    assert_eq!(kana_to_romaji("でーた。"), "de-ta.");
    assert_eq!(kana_to_romaji("2026ねん"), "2026nen");
    assert_eq!(kana_to_romaji("漢"), "漢");
}

#[test]
fn test_unmatched_pending_is_kept_as_letters() {
    assert_eq!(convert_all("rust"), "るst");
    assert_eq!(convert_all("ksa"), "kさ");
    assert_eq!(convert_all("hello"), "へっぉ");
}

// ---- z 記号 (mozc 準拠) ----

#[test]
fn test_z_symbols() {
    assert_eq!(convert_all("zh"), "←");
    assert_eq!(convert_all("zj"), "↓");
    assert_eq!(convert_all("zk"), "↑");
    assert_eq!(convert_all("zl"), "→");
    assert_eq!(convert_all("z."), "…");
    assert_eq!(convert_all("z,"), "‥");
    assert_eq!(convert_all("z-"), "〜");
    assert_eq!(convert_all("z/"), "・");
    assert_eq!(convert_all("z["), "『");
    assert_eq!(convert_all("z]"), "』");
    // 通常の z 行はそのまま
    assert_eq!(convert_all("za"), "ざ");
    assert_eq!(convert_all("zyo"), "じょ");
    assert_eq!(convert_all("mazi"), "まじ");
}

/// mozc の romanji-hiragana.tsv にある綴りは英字を残さず打てる
#[test]
fn test_mozc_spellings() {
    let cases = [
        ("cacicuceco", "かしくせこ"),
        ("cyacyicyucyecyo", "ちゃちぃちゅちぇちょ"),
        ("qaqiquqeqo", "くぁくぃくくぇくぉ"),
        ("kwakwikwukwekwo", "くぁくぃくぅくぇくぉ"),
        ("gwagwigwugwegwo", "ぐぁぐぃぐぅぐぇぐぉ"),
        ("swaswiswusweswo", "すぁすぃすぅすぇすぉ"),
        ("zwazwizwuzwezwo", "ずぁずぃずぅずぇずぉ"),
        ("twatwitwutwetwo", "とぁとぃとぅとぇとぉ"),
        ("dwadwidwudwedwo", "どぁどぃどぅどぇどぉ"),
        ("hwahwihwehwohwyu", "ふぁふぃふぇふぉふゅ"),
        ("whawhiwhuwhewho", "うぁうぃううぇうぉ"),
        ("wu", "う"),
        ("vyavyivyuvyevyo", "ゔゃゔぃゔゅゔぇゔょ"),
        ("xyixyelyilye", "ぃぇぃぇ"),
        ("xn", "ん"),
        ("d'id'ud'yu", "でぃどぅでゅ"),
        ("t'it'ut'yu", "てぃとぅてゅ"),
    ];
    for (input, expected) in cases {
        assert_eq!(convert_all(input), expected, "input: {input}");
    }
}

#[test]
fn test_sokuon_all_consonants() {
    assert_eq!(convert_all("qqa"), "っくぁ");
    assert_eq!(convert_all("yya"), "っや");
    assert_eq!(convert_all("xxa"), "っぁ");
    assert_eq!(convert_all("lla"), "っぁ");
    assert_eq!(convert_all("matcha"), "まっちゃ");
}

#[test]
fn test_n_before_q() {
    assert_eq!(convert_all("nqa"), "んくぁ");
}

/// "www" は草の英字として残し、"wwa" は従来どおり促音にする
#[test]
fn test_repeated_w() {
    assert_eq!(convert_all("www"), "www");
    assert_eq!(convert_all("wwwww"), "wwwww");
    assert_eq!(convert_all("wwa"), "っわ");
    assert_eq!(convert_all("wwwa"), "wっわ");
    assert_eq!(convert_all("ww"), "ww");
}

/// 綴りの途中で記号が来たら打った英字を残す
#[test]
fn test_apostrophe_without_match() {
    assert_eq!(convert_all("d'a"), "d'あ");
    assert_eq!(convert_all("t'"), "t'");
}

/// 追加の綴りが逆変換の優先綴りを奪わない
#[test]
fn test_reverse_keeps_preferred_spelling() {
    assert_eq!(kana_to_romaji("かしくせこ"), "kashikuseko");
    assert_eq!(kana_to_romaji("ちゃてぃふぁうんぅ"), "chathifaunxu");
}

/// 新しい綴りのかなも逆変換して元のかなに戻る
#[test]
fn test_reverse_round_trip_new_kana() {
    let kana = "くぁぐぁとぅどぅうぁすぁずぁゔゃっぁ";
    assert_eq!(convert_all(&kana_to_romaji(kana)), kana);
}
