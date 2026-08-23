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
    fn test_capitalize() {
        assert_eq!(capitalize("rUST"), "Rust");
        assert_eq!(capitalize(""), "");
    }
}
