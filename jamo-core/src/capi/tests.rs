use super::*;

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn read_str(p: *const c_char) -> String {
    assert!(!p.is_null());
    unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string()
}

#[test]
fn test_session_new_rejects_null_engine() {
    assert!(unsafe { jamo_session_new(ptr::null()) }.is_null());
}

#[test]
fn test_engine_open_rejects_bad_path() {
    let p = cstr("/nonexistent/dict.bin");
    assert!(unsafe { jamo_engine_open(p.as_ptr()) }.is_null());
}

/// engine 系のテストは実辞書に依存するため、無ければ skip
/// (ipadic::tests::test_convert_with_ipadic と同じ流儀)
fn open_test_engine() -> Option<*mut JamoCEngine> {
    let dict_path = Path::new("../data/ipadic.dict");
    if !dict_path.exists() {
        eprintln!("Skipping: ipadic.dict not found");
        return None;
    }
    let p = cstr("../data/ipadic.dict");
    let e = unsafe { jamo_engine_open(p.as_ptr()) };
    assert!(!e.is_null());
    Some(e)
}

#[test]
fn test_capi_session_full_flow() {
    let Some(e) = open_test_engine() else { return };
    unsafe {
        let s = jamo_session_new(e);
        assert!(!s.is_null());
        assert_eq!(jamo_session_state(s), JAMO_STATE_INPUT);

        // ローマ字入力
        let input = cstr("kyouha");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(read_str(jamo_session_preedit(s)), "きょうは");

        // 変換
        assert_eq!(jamo_session_convert(s, 5), JAMO_OK);
        assert_eq!(jamo_session_state(s), JAMO_STATE_CONVERTING);
        let count = jamo_session_clause_count(s);
        assert!(count >= 1);
        assert_eq!(jamo_session_focus(s), 0);

        // 候補アクセス
        let n_cands = jamo_session_candidate_count(s, 0);
        assert!(n_cands >= 1);
        assert!(!jamo_session_candidate(s, 0, 0).is_null());
        assert!(jamo_session_candidate(s, 0, n_cands as u32).is_null());
        assert_eq!(jamo_session_selected(s, 0), 0);

        // 候補選択が preedit に反映される
        if n_cands >= 2 {
            assert_eq!(jamo_session_select(s, 0, 1), JAMO_OK);
            let cand = read_str(jamo_session_candidate(s, 0, 1));
            assert!(read_str(jamo_session_preedit(s)).starts_with(&cand));
        }

        // フォーカス範囲
        let (mut from, mut len) = (u32::MAX, u32::MAX);
        assert_eq!(jamo_session_focused_range(s, &mut from, &mut len), JAMO_OK);
        assert_eq!(from, 0);
        assert!(len > 0);

        // 確定でリセットされる
        let committed = read_str(jamo_session_commit(s));
        assert!(!committed.is_empty());
        assert_eq!(jamo_session_state(s), JAMO_STATE_INPUT);
        assert_eq!(read_str(jamo_session_preedit(s)), "");

        jamo_session_free(s);
        jamo_engine_free(e);
    }
}

#[test]
fn test_capi_session_resize_and_revert() {
    let Some(e) = open_test_engine() else { return };
    unsafe {
        let s = jamo_session_new(e);
        let input = cstr("kyouhaiitenki");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(jamo_session_convert(s, 5), JAMO_OK);

        let count = jamo_session_clause_count(s);
        if count >= 2 {
            let yomi_before = read_str(jamo_session_clause_yomi(s, 0));
            assert_eq!(jamo_session_resize(s, 1, 5), JAMO_OK);
            let yomi_after = read_str(jamo_session_clause_yomi(s, 0));
            assert!(yomi_after.chars().count() > yomi_before.chars().count());
        }

        // revert でかなに戻る
        assert_eq!(jamo_session_revert(s), JAMO_OK);
        assert_eq!(jamo_session_state(s), JAMO_STATE_INPUT);
        assert_eq!(read_str(jamo_session_preedit(s)), "きょうはいいてんき");

        jamo_session_free(s);
        jamo_engine_free(e);
    }
}

#[test]
fn test_capi_learning_across_sessions() {
    let Some(e) = open_test_engine() else { return };
    unsafe {
        // 1回目: 候補2を選んで確定
        let s = jamo_session_new(e);
        let input = cstr("kyouha");
        jamo_session_input(s, input.as_ptr());
        jamo_session_convert(s, 5);
        if jamo_session_candidate_count(s, 0) < 2 {
            jamo_session_free(s);
            jamo_engine_free(e);
            return;
        }
        jamo_session_select(s, 0, 1);
        let picked = read_str(jamo_session_candidate(s, 0, 1));
        let committed = read_str(jamo_session_commit(s));
        assert!(committed.starts_with(&picked) || committed.contains(&picked));
        jamo_session_free(s);

        // 2回目: 学習により選んだ候補が先頭に来る
        let s2 = jamo_session_new(e);
        jamo_session_input(s2, input.as_ptr());
        jamo_session_convert(s2, 5);
        let top = read_str(jamo_session_candidate(s2, 0, 0));
        assert_eq!(top, picked, "学習した候補が先頭のはず");

        // 保存 → 読み込みのラウンドトリップ
        let dir = std::env::temp_dir();
        let path = dir.join(format!("jamo-capi-learning-{}.tsv", std::process::id()));
        let cpath = cstr(path.to_str().unwrap());
        assert_eq!(jamo_engine_save_user_data(e, cpath.as_ptr()), JAMO_OK);
        assert_eq!(jamo_engine_load_user_data(e, cpath.as_ptr()), JAMO_OK);
        std::fs::remove_file(&path).ok();

        jamo_session_free(s2);
        jamo_engine_free(e);
    }
}

#[test]
fn test_capi_session_errors() {
    let Some(e) = open_test_engine() else { return };
    unsafe {
        let s = jamo_session_new(e);
        // 空入力の変換はエラー
        assert_eq!(jamo_session_convert(s, 5), JAMO_ERR_EMPTY);
        // Input 中の revert はエラー
        assert_eq!(jamo_session_revert(s), JAMO_ERR_STATE);
        // 変換中の feed はエラー
        let input = cstr("kyouha");
        jamo_session_input(s, input.as_ptr());
        jamo_session_convert(s, 5);
        assert_eq!(jamo_session_feed(s, 'a' as u32), JAMO_ERR_STATE);
        // 範囲外アクセス
        assert_eq!(jamo_session_selected(s, 999), JAMO_ERR_INDEX);
        assert!(jamo_session_clause_yomi(s, 999).is_null());

        jamo_session_free(s);
        jamo_engine_free(e);
    }
}

#[test]
fn test_cursor_api_edits_preedit() {
    let Some(engine) = open_test_engine() else {
        return;
    };
    unsafe {
        let s = jamo_session_new(engine);
        let input = cstr("aiu");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(jamo_session_cursor(s), 3);
        assert_eq!(jamo_session_move_cursor(s, -1), 1);
        assert_eq!(jamo_session_cursor(s), 2);
        assert_eq!(jamo_session_backspace(s), 1);
        assert_eq!(read_str(jamo_session_preedit(s)), "あう");
        let mut from = 99u32;
        let mut len = 99u32;
        assert_eq!(jamo_session_focused_range(s, &mut from, &mut len), JAMO_OK);
        assert_eq!((from, len), (1, 0));
        assert_eq!(jamo_session_delete_forward(s), 1);
        assert_eq!(read_str(jamo_session_preedit(s)), "あ");
        assert_eq!(jamo_session_delete_forward(s), 0);
        assert_eq!(jamo_session_set_cursor(s, 0), JAMO_OK);
        assert_eq!(jamo_session_move_cursor(s, -1), 0);
        assert_eq!(jamo_session_cursor(s), 0);
        jamo_session_free(s);
        jamo_engine_free(engine);
    }
}

#[test]
fn test_alpha_and_transform_api() {
    let Some(engine) = open_test_engine() else {
        return;
    };
    unsafe {
        let s = jamo_session_new(engine);
        let input = cstr("Rust");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(jamo_session_is_alpha(s), 1);
        assert_eq!(read_str(jamo_session_preedit(s)), "Rust");
        assert_eq!(read_str(jamo_session_commit(s)), "Rust");
        assert_eq!(jamo_session_is_alpha(s), 0);

        let input = cstr("jamo");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(jamo_session_transform(s, JAMO_TRANSFORM_KATAKANA), JAMO_OK);
        assert_eq!(jamo_session_state(s), JAMO_STATE_CONVERTING);
        assert_eq!(read_str(jamo_session_preedit(s)), "ジャモ");
        assert_eq!(
            jamo_session_transform(s, JAMO_TRANSFORM_HALF_ALPHA),
            JAMO_OK
        );
        assert_eq!(read_str(jamo_session_preedit(s)), "jamo");
        assert_eq!(
            jamo_session_transform(s, JAMO_TRANSFORM_HALF_ALPHA),
            JAMO_OK
        );
        assert_eq!(read_str(jamo_session_preedit(s)), "JAMO");
        assert_eq!(jamo_session_transform(s, 99), JAMO_ERR);
        jamo_session_reset(s);
        assert_eq!(
            jamo_session_transform(s, JAMO_TRANSFORM_KATAKANA),
            JAMO_ERR_EMPTY
        );
        assert_eq!(jamo_session_set_alpha(s, 1), JAMO_OK);
        let input = cstr("abc");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(read_str(jamo_session_preedit(s)), "abc");
        jamo_session_free(s);
        jamo_engine_free(engine);
    }
}

#[test]
fn test_commit_prefix_api() {
    let Some(engine) = open_test_engine() else {
        return;
    };
    unsafe {
        let s = jamo_session_new(engine);
        let input = cstr("kyouhatenkidesune");
        assert_eq!(jamo_session_input(s, input.as_ptr()), JAMO_OK);
        assert_eq!(jamo_session_convert(s, 5), JAMO_OK);
        let n = jamo_session_clause_count(s);
        assert!(n >= 2, "clauses: {n}");
        let first = read_str(jamo_session_commit_prefix(s));
        assert!(!first.is_empty());
        assert_eq!(jamo_session_state(s), JAMO_STATE_CONVERTING);
        assert_eq!(jamo_session_clause_count(s), n - 1);
        assert_eq!(jamo_session_focus(s), 0);
        jamo_session_reset(s);
        jamo_session_free(s);
        jamo_engine_free(engine);
    }
}
