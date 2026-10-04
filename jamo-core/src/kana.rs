//! かな・文字種の変換ユーティリティ (ひらがな⇔カタカナ、半角⇔全角英数)

/// ひらがなをカタカナに変換 (ひらがな以外はそのまま)
pub fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            // ひらがな (U+3041..U+3096) → カタカナ (U+30A1..U+30F6)
            if (0x3041..=0x3096).contains(&cp) {
                char::from_u32(cp + 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// カタカナをひらがなに変換 (カタカナ以外はそのまま)
pub fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| {
            let cp = c as u32;
            if (0x30A1..=0x30F6).contains(&cp) {
                char::from_u32(cp - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// ASCII の印字可能文字を全角に変換 (それ以外はそのまま)。空白は全角空白
pub fn to_wide(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 - 0x21 + 0xFF01).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 記号だけの文字列の全角/半角を反転する (和文句読点は打鍵した ASCII 記号に戻す)。
/// 記号以外を含む・空なら None
pub fn flip_symbol_width(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    s.chars()
        .map(|c| {
            if c.is_ascii_punctuation() {
                return Some(to_wide(&c.to_string()));
            }
            let narrow = char::from_u32((c as u32).wrapping_sub(0xFF01 - 0x21));
            match narrow {
                Some(n) if n.is_ascii_punctuation() => Some(n.to_string()),
                _ => crate::romaji::punct_to_ascii(c).map(String::from),
            }
        })
        .collect()
}

/// 先頭だけ大文字、残りは小文字 (ASCII のみ)
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_katakana_roundtrip() {
        assert_eq!(to_katakana("きょうはーabc"), "キョウハーabc");
        assert_eq!(to_hiragana("キョウハーabc"), "きょうはーabc");
    }

    #[test]
    fn test_wide() {
        assert_eq!(to_wide("Rust 1.0!"), "Ｒｕｓｔ　１．０！");
        assert_eq!(to_wide("あ"), "あ");
    }

    #[test]
    fn test_flip_symbol_width() {
        assert_eq!(flip_symbol_width("？").as_deref(), Some("?"));
        assert_eq!(flip_symbol_width("！？").as_deref(), Some("!?"));
        assert_eq!(flip_symbol_width("(").as_deref(), Some("（"));
        assert_eq!(flip_symbol_width("（").as_deref(), Some("("));
        // 和文句読点は ASCII の打鍵記号へ
        assert_eq!(flip_symbol_width("。").as_deref(), Some("."));
        assert_eq!(flip_symbol_width("「」").as_deref(), Some("[]"));
        assert_eq!(flip_symbol_width("〜").as_deref(), Some("~"));
        // 記号以外を含むものは対象外
        assert_eq!(flip_symbol_width("何？"), None);
        assert_eq!(flip_symbol_width("1"), None);
        assert_eq!(flip_symbol_width(""), None);
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("rUST"), "Rust");
        assert_eq!(capitalize(""), "");
    }
}
