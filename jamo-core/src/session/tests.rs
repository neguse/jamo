use super::*;
use crate::dict::HashMapDict;
use crate::viterbi::SimpleConnectionCost;

fn feed_str(s: &mut Session, text: &str) {
    for ch in text.chars() {
        s.feed(ch).unwrap();
    }
}

fn converting_session(input: &str) -> (Session, HashMapDict) {
    let dict = HashMapDict::test_dict();
    let mut s = Session::new();
    feed_str(&mut s, input);
    s.convert(&dict, &SimpleConnectionCost, 5, None).unwrap();
    (s, dict)
}

#[test]
fn test_new_session_is_empty_input() {
    let s = Session::new();
    assert_eq!(s.state(), SessionState::Input);
    assert_eq!(s.preedit(), "");
    assert!(s.clauses().is_empty());
}

#[test]
fn test_feed_romaji_builds_kana() {
    let mut s = Session::new();
    s.feed('k').unwrap();
    assert_eq!(s.preedit(), "k"); // 未確定ローマ字も preedit に出る
    s.feed('y').unwrap();
    s.feed('o').unwrap();
    assert_eq!(s.preedit(), "きょ");
    s.feed('u').unwrap();
    assert_eq!(s.preedit(), "きょう");
}

#[test]
fn test_feed_hiragana_passthrough() {
    let mut s = Session::new();
    s.feed('き').unwrap();
    s.feed('ょ').unwrap();
    assert_eq!(s.preedit(), "きょ");
}

#[test]
fn test_backspace_pending_then_kana() {
    let mut s = Session::new();
    feed_str(&mut s, "kak");
    assert_eq!(s.preedit(), "かk");
    assert_eq!(s.backspace(), Ok(true)); // pending "k" を消す
    assert_eq!(s.preedit(), "か");
    assert_eq!(s.backspace(), Ok(true)); // かなを消す
    assert_eq!(s.preedit(), "");
    assert_eq!(s.backspace(), Ok(false)); // もう何もない
}

#[test]
fn test_convert_moves_to_converting() {
    let (s, _dict) = converting_session("kyouha");
    assert_eq!(s.state(), SessionState::Converting);
    assert_eq!(s.clauses().len(), 1);
    assert_eq!(s.clauses()[0].yomi, "きょうは");
    assert_eq!(s.preedit(), "今日は");
    assert_eq!(s.focus(), 0);
}

#[test]
fn test_convert_flushes_pending_n() {
    let dict = HashMapDict::test_dict();
    let mut s = Session::new();
    feed_str(&mut s, "kyoun"); // "n" が pending に残る
    s.convert(&dict, &SimpleConnectionCost, 5, None).unwrap();
    let all_yomi: String = s.clauses().iter().map(|c| c.yomi.as_str()).collect();
    assert_eq!(all_yomi, "きょうん"); // flush で「ん」が確定してから変換される
}

#[test]
fn test_convert_empty_is_error() {
    let dict = HashMapDict::test_dict();
    let mut s = Session::new();
    assert_eq!(
        s.convert(&dict, &SimpleConnectionCost, 5, None),
        Err(SessionError::EmptyInput)
    );
}

#[test]
fn test_feed_while_converting_is_error() {
    let (mut s, _dict) = converting_session("kyouha");
    assert_eq!(s.feed('a'), Err(SessionError::InvalidState));
}

#[test]
fn test_select_candidate_changes_preedit() {
    let (mut s, _dict) = converting_session("kyouha");
    let candidates = s.clauses()[0].candidates.clone();
    let kyou_ha = candidates.iter().position(|c| c == "京は").unwrap();
    s.select(0, kyou_ha).unwrap();
    assert_eq!(s.preedit(), "京は");
    assert_eq!(s.clauses()[0].selected, kyou_ha);
    assert_eq!(s.select(0, 999), Err(SessionError::InvalidIndex));
}

#[test]
fn test_focus_and_range() {
    // きょうはてんき → [今日は][天][木]
    let (mut s, _dict) = converting_session("kyouhatenki");
    assert_eq!(s.clauses().len(), 3);
    assert_eq!(s.focused_range(), (0, 3)); // 今日は
    s.set_focus(1).unwrap();
    assert_eq!(s.focused_range(), (3, 1)); // 天
    assert_eq!(s.set_focus(99), Err(SessionError::InvalidIndex));
}

#[test]
fn test_resize_extends_focused_clause() {
    // [今日は][天][木] の文節0を1文字伸ばす → 読み「きょうはて」
    let (mut s, dict) = converting_session("kyouhatenki");
    s.resize(1, &dict, &SimpleConnectionCost, 5, None).unwrap();
    assert_eq!(s.clauses()[0].yomi, "きょうはて");
    assert_eq!(s.focus(), 0);
    // 全文節の読みを連結すると元のかなに戻る
    let all: String = s.clauses().iter().map(|c| c.yomi.as_str()).collect();
    assert_eq!(all, "きょうはてんき");
}

#[test]
fn test_resize_rejects_last_clause_extend() {
    let (mut s, dict) = converting_session("kyouha");
    // 1文節のみ → 最後の文節は伸縮不可
    assert_eq!(
        s.resize(1, &dict, &SimpleConnectionCost, 5, None),
        Err(SessionError::InvalidResize)
    );
}

#[test]
fn test_revert_returns_to_input_with_kana() {
    let (mut s, _dict) = converting_session("kyouha");
    s.revert().unwrap();
    assert_eq!(s.state(), SessionState::Input);
    assert_eq!(s.preedit(), "きょうは");
}

#[test]
fn test_commit_converting_returns_selection_and_resets() {
    let (mut s, _dict) = converting_session("kyouha");
    assert_eq!(s.commit(None), "今日は");
    assert_eq!(s.state(), SessionState::Input);
    assert_eq!(s.preedit(), "");
}

#[test]
fn test_commit_input_returns_kana_with_flush() {
    let mut s = Session::new();
    feed_str(&mut s, "kyoun"); // pending "n"
    assert_eq!(s.commit(None), "きょうん");
    assert_eq!(s.preedit(), "");
}

#[test]
fn test_convert_applies_learning() {
    let dict = HashMapDict::test_dict();
    let mut store = LearningStore::new(16);
    store.record("きょうは", "京は");
    let mut s = Session::new();
    feed_str(&mut s, "kyouha");
    s.convert(&dict, &SimpleConnectionCost, 5, Some(&store))
        .unwrap();
    assert_eq!(s.clauses()[0].candidates[0], "京は");
    assert_eq!(s.preedit(), "京は");
}

#[test]
fn test_commit_records_learning_and_next_convert_uses_it() {
    let dict = HashMapDict::test_dict();
    let mut store = LearningStore::new(16);
    let (mut s, _dict) = converting_session("kyouha");
    let idx = s.clauses()[0]
        .candidates
        .iter()
        .position(|c| c == "京は")
        .unwrap();
    s.select(0, idx).unwrap();
    s.commit(Some(&mut store));
    assert_eq!(store.get("きょうは"), Some("京は"));

    let mut s2 = Session::new();
    feed_str(&mut s2, "kyouha");
    s2.convert(&dict, &SimpleConnectionCost, 5, Some(&store))
        .unwrap();
    assert_eq!(s2.preedit(), "京は");
}

#[test]
fn test_reset_clears_everything() {
    let (mut s, _dict) = converting_session("kyouha");
    s.reset();
    assert_eq!(s.state(), SessionState::Input);
    assert_eq!(s.preedit(), "");
    assert!(s.clauses().is_empty());
}

// ---- T161: preedit カーソル ----

#[test]
fn test_cursor_starts_at_end_and_moves() {
    let mut s = Session::new();
    feed_str(&mut s, "kyou");
    assert_eq!(s.cursor(), 3);
    assert_eq!(s.focused_range(), (3, 0));
    assert!(s.move_cursor(-1).unwrap());
    assert_eq!(s.cursor(), 2);
    assert!(s.move_cursor(-5).unwrap()); // 端に丸める
    assert_eq!(s.cursor(), 0);
    assert!(!s.move_cursor(-1).unwrap()); // 端では動かない
    assert!(s.move_cursor(1).unwrap());
    assert_eq!(s.cursor(), 1);
    s.set_cursor(99).unwrap();
    assert_eq!(s.cursor(), 3);
}

#[test]
fn test_insert_at_cursor() {
    let mut s = Session::new();
    feed_str(&mut s, "kyu"); // きゅ
    s.move_cursor(-1).unwrap(); // き|ゅ
    feed_str(&mut s, "ka");
    assert_eq!(s.preedit(), "きかゅ");
    assert_eq!(s.cursor(), 2);
}

#[test]
fn test_pending_shows_at_cursor() {
    let mut s = Session::new();
    feed_str(&mut s, "ai");
    s.move_cursor(-1).unwrap();
    s.feed('k').unwrap();
    assert_eq!(s.preedit(), "あkい");
    assert_eq!(s.cursor(), 2);
    s.feed('a').unwrap();
    assert_eq!(s.preedit(), "あかい");
}

#[test]
fn test_move_cursor_flushes_pending() {
    let mut s = Session::new();
    feed_str(&mut s, "kan"); // か + pending n
    s.move_cursor(-1).unwrap();
    assert_eq!(s.preedit(), "かん");
    assert_eq!(s.cursor(), 1);
    let mut s = Session::new();
    feed_str(&mut s, "kak"); // か + pending k
    s.move_cursor(-1).unwrap();
    assert_eq!(s.preedit(), "かk"); // 未完成の子音は英字のまま残る
}

#[test]
fn test_backspace_and_delete_at_cursor() {
    let mut s = Session::new();
    feed_str(&mut s, "aiu");
    s.move_cursor(-1).unwrap(); // あい|う
    assert!(s.backspace().unwrap());
    assert_eq!(s.preedit(), "あう");
    assert_eq!(s.cursor(), 1);
    assert!(s.delete_forward().unwrap());
    assert_eq!(s.preedit(), "あ");
    assert!(!s.delete_forward().unwrap()); // 末尾では消すものがない
    s.set_cursor(0).unwrap();
    assert!(!s.backspace().unwrap()); // 先頭では消すものがない
    assert_eq!(s.preedit(), "あ");
}

#[test]
fn test_convert_uses_whole_kana_regardless_of_cursor() {
    let dict = HashMapDict::test_dict();
    let mut s = Session::new();
    feed_str(&mut s, "kyouha");
    s.set_cursor(1).unwrap();
    s.convert(&dict, &SimpleConnectionCost, 5, None).unwrap();
    assert_eq!(s.preedit(), "今日は");
    s.revert().unwrap();
    assert_eq!(s.cursor(), 4); // revert 後のカーソルは末尾
}

#[test]
fn test_cursor_ops_rejected_while_converting() {
    let (mut s, _dict) = converting_session("kyouha");
    assert_eq!(s.move_cursor(-1), Err(SessionError::InvalidState));
    assert_eq!(s.delete_forward(), Err(SessionError::InvalidState));
}
