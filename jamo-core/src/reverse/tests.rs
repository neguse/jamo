use super::*;
use crate::dict::{HashMapDict, POS_CONTENT, POS_FUNCTION};
use crate::viterbi::SimpleConnectionCost;

fn words(text: &str, dict: &HashMapDict) -> Vec<(String, String)> {
    reading(text, dict, &SimpleConnectionCost)
        .into_iter()
        .map(|w| (w.surface, w.reading))
        .collect()
}

fn pair(s: &str, r: &str) -> (String, String) {
    (s.to_string(), r.to_string())
}

/// 単語コストが正の小さな辞書 (語が少ないほど安い)
fn dict() -> HashMapDict {
    let mut dict = HashMapDict::new();
    dict.add("きょう", "今日", 0, 0, 2000, POS_CONTENT);
    dict.add("こんにち", "今日", 0, 0, 4000, POS_CONTENT);
    dict.add("は", "は", 1, 1, 500, POS_FUNCTION);
    dict.add("てんき", "天気", 0, 0, 3000, POS_CONTENT);
    dict.add("てん", "天", 0, 0, 2000, POS_CONTENT);
    dict.add("き", "気", 0, 0, 2000, POS_CONTENT);
    dict.add("です", "です", 4, 4, 500, POS_FUNCTION);
    dict
}

#[test]
fn test_reading_kanji_and_particle() {
    assert_eq!(
        words("今日は天気です", &dict()),
        vec![
            pair("今日", "きょう"),
            pair("は", "は"),
            pair("天気", "てんき"),
            pair("です", "です"),
        ]
    );
}

#[test]
fn test_reading_prefers_cheaper_word() {
    // 天気 (てんき) は 天 + 気 より安いので 1 語で読む
    assert_eq!(words("天気", &dict()), vec![pair("天気", "てんき")]);
}

#[test]
fn test_reading_depends_on_context() {
    // 同じ 間 でも、接続コストで前後の語に合う読みを選ぶ
    let mut dict = HashMapDict::new();
    dict.add("あいだ", "間", 10, 10, 3000, POS_CONTENT);
    dict.add("かん", "間", 20, 20, 1000, POS_CONTENT);
    dict.add("ない", "ない", 30, 30, 1000, POS_FUNCTION);
    dict.add("ねんかん", "年間", 0, 0, 9000, POS_CONTENT);
    dict.add("ねん", "年", 0, 0, 1000, POS_CONTENT);
    struct Conn;
    impl ConnectionCost for Conn {
        fn cost(&self, right_id: u16, left_id: u16) -> i32 {
            match (right_id, left_id) {
                (30, 10) => 0,
                (30, 20) => 5000,
                _ => 0,
            }
        }
    }
    let got: Vec<String> = reading("ない間", &dict, &Conn)
        .into_iter()
        .map(|w| w.reading)
        .collect();
    assert_eq!(got, vec!["ない", "あいだ"]);
    let got: Vec<String> = reading("年間", &dict, &Conn)
        .into_iter()
        .map(|w| w.reading)
        .collect();
    assert_eq!(got, vec!["ねん", "かん"]);
}

#[test]
fn test_reading_unknown_chars() {
    // 辞書に無いかなはひらがなにし、ほかの字はそのまま返す (入力を落とさない)
    assert_eq!(
        words("ピ猫", &dict()),
        vec![pair("ピ", "ぴ"), pair("猫", "猫")]
    );
}

#[test]
fn test_reading_empty() {
    assert!(words("", &dict()).is_empty());
}

#[test]
fn test_readings_matches_reading() {
    let dict = dict();
    let texts = ["今日は天気です", "天気", ""];
    let all = readings(&texts, &dict, &SimpleConnectionCost);
    assert_eq!(all.len(), texts.len());
    for (text, got) in texts.iter().zip(all) {
        assert_eq!(got, reading(text, &dict, &SimpleConnectionCost));
    }
}
