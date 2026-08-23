//! 数字列の表記バリエーション (全角・漢数字・位取り漢数字・桁区切り)

use crate::kana::to_wide;

static KANJI_DIGITS: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
static SMALL_UNITS: [&str; 4] = ["", "十", "百", "千"];
static LARGE_UNITS: [&str; 5] = ["", "万", "億", "兆", "京"];

/// 先頭の ASCII 数字列 (なければ None)
pub fn leading_digits(s: &str) -> Option<&str> {
    let len = s.bytes().take_while(u8::is_ascii_digit).count();
    (len > 0).then(|| &s[..len])
}

/// 各桁を漢数字に置き換える (2026 → 二〇二六)
pub fn kanji_digits(digits: &str) -> String {
    digits
        .chars()
        .map(|c| {
            c.to_digit(10)
                .map(|d| KANJI_DIGITS[d as usize])
                .unwrap_or(c)
        })
        .collect()
}

/// 位取り漢数字 (2026 → 二千二十六)。先頭 0 や 20 桁超は対象外で None
pub fn kanji_positional(digits: &str) -> Option<String> {
    if digits.is_empty() || digits.len() > 20 || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    if digits == "0" {
        return Some("〇".to_string());
    }
    let ds: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    let mut out = String::new();
    // 下位から4桁ずつ区切り、上位の組から並べる
    let groups: Vec<&[u32]> = ds.rchunks(4).collect();
    for (gi, group) in groups.iter().enumerate().rev() {
        let mut part = String::new();
        for (i, &d) in group.iter().enumerate() {
            let unit = group.len() - 1 - i;
            if d == 0 {
                continue;
            }
            // 十・百・千の前の「一」は省く
            if !(d == 1 && unit > 0) {
                part.push(KANJI_DIGITS[d as usize]);
            }
            part.push_str(SMALL_UNITS[unit]);
        }
        if !part.is_empty() {
            out.push_str(&part);
            out.push_str(LARGE_UNITS[gi]);
        }
    }
    Some(out)
}

/// 3桁ごとのカンマ区切り (4桁以上のみ、2026 → 2,026)
pub fn comma_grouped(digits: &str) -> Option<String> {
    if digits.len() < 4 || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    Some(out)
}

/// 数字列の表記候補 (元の半角は含まない、重複なし)
pub fn variants(digits: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: Option<String>| {
        if let Some(s) = s
            && s != digits
            && !out.contains(&s)
        {
            out.push(s);
        }
    };
    push(Some(to_wide(digits)));
    push(Some(kanji_digits(digits)));
    push(kanji_positional(digits));
    push(comma_grouped(digits));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leading_digits() {
        assert_eq!(leading_digits("2026ねん"), Some("2026"));
        assert_eq!(leading_digits("ねん"), None);
    }

    #[test]
    fn test_kanji_digits() {
        assert_eq!(kanji_digits("2026"), "二〇二六");
    }

    #[test]
    fn test_kanji_positional() {
        assert_eq!(kanji_positional("2026").unwrap(), "二千二十六");
        assert_eq!(kanji_positional("10").unwrap(), "十");
        assert_eq!(kanji_positional("111").unwrap(), "百十一");
        assert_eq!(kanji_positional("1000").unwrap(), "千");
        assert_eq!(kanji_positional("10000").unwrap(), "一万");
        assert_eq!(
            kanji_positional("123456789").unwrap(),
            "一億二千三百四十五万六千七百八十九"
        );
        assert_eq!(kanji_positional("0").unwrap(), "〇");
        assert_eq!(kanji_positional("007"), None);
    }

    #[test]
    fn test_comma_grouped() {
        assert_eq!(comma_grouped("2026").unwrap(), "2,026");
        assert_eq!(comma_grouped("1234567").unwrap(), "1,234,567");
        assert_eq!(comma_grouped("123"), None);
    }

    #[test]
    fn test_variants() {
        assert_eq!(
            variants("2026"),
            vec!["２０２６", "二〇二六", "二千二十六", "2,026"]
        );
        assert_eq!(variants("6"), vec!["６", "六"]);
    }
}
