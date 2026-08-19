//! ローマ字→ひらがな変換 FSM
//!
//! ASCII キー入力を受け取り、ひらがなを出力する有限状態機械。
//! 静的ルックアップテーブルで約200のローマ字パターンを網羅。

/// ローマ字テーブル: (ローマ字, ひらがな)
/// 長いパターンを先に検索するためソート済み
static ROMAJI_TABLE: &[(&str, &str)] = &[
    // 拗音 (3文字)
    ("wyi", "ゐ"),
    ("wye", "ゑ"),
    ("kya", "きゃ"),
    ("kyi", "きぃ"),
    ("kyu", "きゅ"),
    ("kye", "きぇ"),
    ("kyo", "きょ"),
    ("sha", "しゃ"),
    ("shi", "し"),
    ("shu", "しゅ"),
    ("she", "しぇ"),
    ("sho", "しょ"),
    ("sya", "しゃ"),
    ("syi", "しぃ"),
    ("syu", "しゅ"),
    ("sye", "しぇ"),
    ("syo", "しょ"),
    ("cha", "ちゃ"),
    ("chi", "ち"),
    ("chu", "ちゅ"),
    ("che", "ちぇ"),
    ("cho", "ちょ"),
    ("tya", "ちゃ"),
    ("tyi", "ちぃ"),
    ("tyu", "ちゅ"),
    ("tye", "ちぇ"),
    ("tyo", "ちょ"),
    ("tha", "てゃ"),
    ("thi", "てぃ"),
    ("thu", "てゅ"),
    ("the", "てぇ"),
    ("tho", "てょ"),
    ("tsa", "つぁ"),
    ("tsi", "つぃ"),
    ("tsu", "つ"),
    ("tse", "つぇ"),
    ("tso", "つぉ"),
    ("nya", "にゃ"),
    ("nyi", "にぃ"),
    ("nyu", "にゅ"),
    ("nye", "にぇ"),
    ("nyo", "にょ"),
    ("hya", "ひゃ"),
    ("hyi", "ひぃ"),
    ("hyu", "ひゅ"),
    ("hye", "ひぇ"),
    ("hyo", "ひょ"),
    ("mya", "みゃ"),
    ("myi", "みぃ"),
    ("myu", "みゅ"),
    ("mye", "みぇ"),
    ("myo", "みょ"),
    ("rya", "りゃ"),
    ("ryi", "りぃ"),
    ("ryu", "りゅ"),
    ("rye", "りぇ"),
    ("ryo", "りょ"),
    ("gya", "ぎゃ"),
    ("gyi", "ぎぃ"),
    ("gyu", "ぎゅ"),
    ("gye", "ぎぇ"),
    ("gyo", "ぎょ"),
    ("zya", "じゃ"),
    ("zyi", "じぃ"),
    ("zyu", "じゅ"),
    ("zye", "じぇ"),
    ("zyo", "じょ"),
    ("jya", "じゃ"),
    ("jyi", "じぃ"),
    ("jyu", "じゅ"),
    ("jye", "じぇ"),
    ("jyo", "じょ"),
    ("dya", "ぢゃ"),
    ("dyi", "ぢぃ"),
    ("dyu", "ぢゅ"),
    ("dye", "ぢぇ"),
    ("dyo", "ぢょ"),
    ("dha", "でゃ"),
    ("dhi", "でぃ"),
    ("dhu", "でゅ"),
    ("dhe", "でぇ"),
    ("dho", "でょ"),
    ("bya", "びゃ"),
    ("byi", "びぃ"),
    ("byu", "びゅ"),
    ("bye", "びぇ"),
    ("byo", "びょ"),
    ("pya", "ぴゃ"),
    ("pyi", "ぴぃ"),
    ("pyu", "ぴゅ"),
    ("pye", "ぴぇ"),
    ("pyo", "ぴょ"),
    ("fya", "ふゃ"),
    ("fyu", "ふゅ"),
    ("fyo", "ふょ"),
    // f+母音
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fu", "ふ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    // 小文字 (x/l プレフィックス, 3文字)
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("xtu", "っ"),
    ("xwa", "ゎ"),
    ("xka", "ヵ"),
    ("xke", "ヶ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
    ("ltu", "っ"),
    ("lwa", "ゎ"),
    ("lka", "ヵ"),
    ("lke", "ヶ"),
    // j+母音
    ("ja", "じゃ"),
    ("ji", "じ"),
    ("ju", "じゅ"),
    ("je", "じぇ"),
    ("jo", "じょ"),
    // 清音 (2文字)
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("sa", "さ"),
    ("si", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("ta", "た"),
    ("ti", "ち"),
    ("tu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("ha", "は"),
    ("hi", "ひ"),
    ("hu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("ya", "や"),
    ("yi", "い"),
    ("yu", "ゆ"),
    ("ye", "いぇ"),
    ("yo", "よ"),
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("wa", "わ"),
    ("wi", "うぃ"),
    ("we", "うぇ"),
    ("wo", "を"),
    // 濁音 (2文字)
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("za", "ざ"),
    ("zi", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    // 半濁音 (2文字)
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    // ヴ
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    // 小文字 (x/l, 2文字)
    ("xa", "ぁ"),
    ("xi", "ぃ"),
    ("xu", "ぅ"),
    ("xe", "ぇ"),
    ("xo", "ぉ"),
    ("la", "ぁ"),
    ("li", "ぃ"),
    ("lu", "ぅ"),
    ("le", "ぇ"),
    ("lo", "ぉ"),
    // 単独母音 (1文字)
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    // ん (単体)
    ("n", "ん"),
];

/// 句読点テーブル
static PUNCTUATION_TABLE: &[(char, &str)] = &[
    ('.', "。"),
    (',', "、"),
    ('/', "・"),
    ('[', "「"),
    (']', "」"),
    ('-', "ー"),
    ('!', "！"),
    ('?', "？"),
    ('~', "〜"),
];

/// 母音かどうか
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'i' | 'u' | 'e' | 'o')
}

/// ローマ字の子音として有効な文字かどうか
fn is_romaji_consonant(c: char) -> bool {
    matches!(
        c,
        'k' | 's'
            | 't'
            | 'n'
            | 'h'
            | 'm'
            | 'y'
            | 'r'
            | 'w'
            | 'g'
            | 'z'
            | 'd'
            | 'b'
            | 'p'
            | 'c'
            | 'f'
            | 'j'
            | 'v'
            | 'x'
            | 'l'
    )
}

/// 促音を生成する子音の重複パターンかどうか
fn is_double_consonant(c: char) -> bool {
    matches!(
        c,
        'k' | 's'
            | 't'
            | 'h'
            | 'm'
            | 'r'
            | 'w'
            | 'g'
            | 'z'
            | 'd'
            | 'b'
            | 'p'
            | 'c'
            | 'f'
            | 'j'
            | 'v'
    )
}

/// テーブルからマッチを探す
/// 完全一致があれば Some((ひらがな, 消費文字数))、なければ None
fn lookup_exact(s: &str) -> Option<(&'static str, usize)> {
    for &(roma, hira) in ROMAJI_TABLE {
        if s == roma {
            return Some((hira, roma.len()));
        }
    }
    None
}

/// テーブルにプレフィックスマッチするパターンがあるか
fn has_prefix_match(s: &str) -> bool {
    for &(roma, _) in ROMAJI_TABLE {
        if roma.starts_with(s) && roma.len() > s.len() {
            return true;
        }
    }
    false
}

/// ローマ字変換結果
#[derive(Debug, Clone, PartialEq)]
pub enum RomajiResult {
    /// ひらがな確定 (複数文字の場合あり: 促音 "っか" など)
    Emit(String),
    /// 入力待ち (まだ確定できない)
    Pending,
}

/// Backspace 処理結果
#[derive(Debug, Clone, PartialEq)]
pub enum BackspaceResult {
    /// pending の末尾を削除した
    DeletedPending,
    /// pending が空 → 上位レイヤーがひらがなを削除すべき
    DeleteHiragana,
    /// 何もない (Direct 状態など)
    Empty,
}

/// ローマ字→ひらがな変換器
#[derive(Debug, Clone)]
pub struct RomajiConverter {
    pending: String,
}

impl RomajiConverter {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// 1文字入力し、ひらがな確定 or 待機を返す
    pub fn feed(&mut self, ch: char) -> RomajiResult {
        // 句読点チェック (pending が空の場合のみ)
        if self.pending.is_empty() {
            for &(punct, hira) in PUNCTUATION_TABLE {
                if ch == punct {
                    return RomajiResult::Emit(hira.to_string());
                }
            }
        }

        // ローマ字でない文字はそのまま返す
        let ch_lower = ch.to_ascii_lowercase();
        if !ch_lower.is_ascii_alphabetic() {
            if self.pending.is_empty() {
                return RomajiResult::Emit(ch.to_string());
            }
            // pending がある場合は flush してから文字を返す
            let flushed = self.flush();
            return RomajiResult::Emit(format!("{flushed}{ch}"));
        }

        let candidate = format!("{}{}", self.pending, ch_lower);

        // "nn" → "ん" (即座に確定)
        if self.pending == "n" && ch_lower == 'n' {
            self.pending.clear();
            return RomajiResult::Emit("ん".to_string());
        }

        // "n" の特殊処理: n の後に母音・y 以外の子音が来たら "ん" + 新しい子音
        if self.pending == "n" && ch_lower != 'y' && !is_vowel(ch_lower) {
            if is_romaji_consonant(ch_lower) {
                self.pending = ch_lower.to_string();
                return RomajiResult::Emit("ん".to_string());
            }
            // ローマ字の子音でもない文字
            self.pending.clear();
            return RomajiResult::Emit(format!("ん{ch_lower}"));
        }

        // "m" + b/p → "ん" (shimbun → しんぶん)
        if self.pending == "m" && matches!(ch_lower, 'b' | 'p') {
            self.pending = ch_lower.to_string();
            return RomajiResult::Emit("ん".to_string());
        }

        // 完全一致チェック
        if let Some((hira, _)) = lookup_exact(&candidate) {
            // "n" 単体マッチは特殊: まだ確定しない (na, ni 等の可能性)
            if candidate == "n" {
                self.pending = candidate;
                return RomajiResult::Pending;
            }
            self.pending.clear();
            return RomajiResult::Emit(hira.to_string());
        }

        // プレフィックスマッチ (まだ長いパターンがありえる)
        if has_prefix_match(&candidate) {
            self.pending = candidate;
            return RomajiResult::Pending;
        }

        // 促音チェック: 同じ子音の連続 (nn は除外: "ん" で処理済み)
        if candidate.len() == 2
            && ch_lower != 'n'
            && is_double_consonant(ch_lower)
            && self.pending.len() == 1
            && self.pending.starts_with(ch_lower)
        {
            self.pending = ch_lower.to_string();
            return RomajiResult::Emit("っ".to_string());
        }

        // マッチしない → pending をリセットして再試行
        if !self.pending.is_empty() {
            // pending を捨てて、新しい文字から再開
            self.pending.clear();
            return self.feed(ch);
        }

        // 単独文字でもマッチしない
        RomajiResult::Pending
    }

    /// Backspace 処理
    pub fn backspace(&mut self) -> BackspaceResult {
        if self.pending.is_empty() {
            BackspaceResult::DeleteHiragana
        } else {
            self.pending.pop();
            BackspaceResult::DeletedPending
        }
    }

    /// pending を強制確定 ("n" → "ん", それ以外は空文字)
    pub fn flush(&mut self) -> String {
        if self.pending == "n" {
            self.pending.clear();
            "ん".to_string()
        } else {
            self.pending.clear();
            String::new()
        }
    }

    /// 現在の未確定ローマ字
    pub fn pending(&self) -> &str {
        &self.pending
    }

    /// 状態クリア
    pub fn reset(&mut self) {
        self.pending.clear();
    }
}

impl Default for RomajiConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
