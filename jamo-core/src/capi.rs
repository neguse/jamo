//! C API バインディング (v2: セッション型)
//!
//! `#[unsafe(no_mangle)] extern "C"` で公開する。cbindgen でヘッダを自動生成。
//!
//! v2 で JSON 文字列を返すステートレス API (jamo_convert/jamo_resize/
//! jamo_predict/jamo_romaji_*) を廃止し、セッションハンドル + アクセサに
//! 置き換えた。呼び出し側の JSON パーサと状態管理を不要にするため。
//!
//! 文字列を返す関数の返値は、同じセッションへの次の呼び出しまで有効
//! (セッション内部のバッファを指す)。呼び出し側での解放は不要。
//! セッションはスレッド間で共有できない。engine はそれを参照する全セッション
//! より長生きさせること。

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::ptr;

use crate::ipadic::IpaDict;
use crate::learning::LearningStore;
use crate::session::{Session, SessionError, SessionState};

/// 成功
pub const JAMO_OK: i32 = 0;
/// 引数不正 (NULL、不正 UTF-8 など)
pub const JAMO_ERR: i32 = -1;
/// この状態では実行できない操作 (変換中の feed など)
pub const JAMO_ERR_STATE: i32 = -2;
/// 文節/候補インデックスが範囲外
pub const JAMO_ERR_INDEX: i32 = -3;
/// 伸縮できない (端の文節など)
pub const JAMO_ERR_RESIZE: i32 = -4;
/// 入力が空
pub const JAMO_ERR_EMPTY: i32 = -5;

/// jamo_session_state の返値: かな入力中
pub const JAMO_STATE_INPUT: i32 = 0;
/// jamo_session_state の返値: 変換中
pub const JAMO_STATE_CONVERTING: i32 = 1;

fn error_code(e: SessionError) -> i32 {
    match e {
        SessionError::InvalidState => JAMO_ERR_STATE,
        SessionError::EmptyInput => JAMO_ERR_EMPTY,
        SessionError::InvalidIndex => JAMO_ERR_INDEX,
        SessionError::InvalidResize => JAMO_ERR_RESIZE,
    }
}

/// 不透明なエンジンハンドル
pub struct JamoCEngine {
    dict: IpaDict,
    /// 変換学習 (エンジン単位で全セッション共有)
    learning: RefCell<LearningStore>,
}

/// バイナリ辞書ファイルからエンジンを生成
///
/// 成功時はハンドルのポインタ、失敗時は NULL を返す。
/// 呼び出し側は使用後に `jamo_engine_free` で解放すること。
///
/// # Safety
/// `dict_path` は有効なヌル終端 UTF-8 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_engine_open(dict_path: *const c_char) -> *mut JamoCEngine {
    if dict_path.is_null() {
        return ptr::null_mut();
    }
    let path = match unsafe { CStr::from_ptr(dict_path) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    match IpaDict::load_binary(Path::new(path)) {
        Ok(dict) => Box::into_raw(Box::new(JamoCEngine {
            dict,
            learning: RefCell::new(LearningStore::default()),
        })),
        Err(_) => ptr::null_mut(),
    }
}

/// エンジンを解放
///
/// # Safety
/// `engine` は `jamo_engine_open` が返した有効なポインタ、または NULL であること。
/// これを参照するセッションを先に解放すること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_engine_free(engine: *mut JamoCEngine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// 不透明なセッションハンドル
pub struct JamoCSession {
    session: Session,
    engine: *const JamoCEngine,
    /// 文字列返却用バッファ (次の呼び出しまで有効)
    scratch: CString,
}

impl JamoCSession {
    fn scratch_str(&mut self, s: String) -> *const c_char {
        match CString::new(s) {
            Ok(c) => {
                self.scratch = c;
                self.scratch.as_ptr()
            }
            Err(_) => ptr::null(),
        }
    }
}

/// セッションを生成
///
/// 呼び出し側は使用後に `jamo_session_free` で解放すること。
///
/// # Safety
/// `engine` は `jamo_engine_open` が返した有効なポインタで、
/// セッションより長生きすること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_new(engine: *const JamoCEngine) -> *mut JamoCSession {
    if engine.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(JamoCSession {
        session: Session::new(),
        engine,
        scratch: CString::default(),
    }))
}

/// セッションを解放
///
/// # Safety
/// `session` は `jamo_session_new` が返した有効なポインタ、または NULL であること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_free(session: *mut JamoCSession) {
    if !session.is_null() {
        drop(unsafe { Box::from_raw(session) });
    }
}

/// 全状態を破棄して初期状態に戻す
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_reset(session: *mut JamoCSession) {
    if let Some(s) = unsafe { session.as_mut() } {
        s.session.reset();
    }
}

/// 現在の状態 (JAMO_STATE_*) を返す
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_state(session: *const JamoCSession) -> i32 {
    match unsafe { session.as_ref() } {
        Some(s) => match s.session.state() {
            SessionState::Input => JAMO_STATE_INPUT,
            SessionState::Converting => JAMO_STATE_CONVERTING,
        },
        None => JAMO_ERR,
    }
}

/// 1文字入力する (Unicode スカラ値)。ローマ字はひらがなに変換される。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_feed(session: *mut JamoCSession, ch: u32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    let Some(ch) = char::from_u32(ch) else {
        return JAMO_ERR;
    };
    match s.session.feed(ch) {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// 文字列をまとめて入力する (UTF-8。ローマ字/かな混在可)
///
/// # Safety
/// `session` は有効なセッションポインタ、`text` は有効なヌル終端 UTF-8
/// 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_input(
    session: *mut JamoCSession,
    text: *const c_char,
) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    if text.is_null() {
        return JAMO_ERR;
    }
    let Ok(text) = (unsafe { CStr::from_ptr(text) }).to_str() else {
        return JAMO_ERR;
    };
    for ch in text.chars() {
        if let Err(e) = s.session.feed(ch) {
            return error_code(e);
        }
    }
    JAMO_OK
}

/// Backspace。1文字消したら 1、消すものがなければ 0 を返す
/// (0 のとき呼び出し側は通常の Backspace として扱う)。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_backspace(session: *mut JamoCSession) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.backspace() {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => error_code(e),
    }
}

/// preedit 表示文字列を返す。
/// Input 中: かな+未確定ローマ字、Converting 中: 選択中候補の連結。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_preedit(session: *mut JamoCSession) -> *const c_char {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return ptr::null();
    };
    let text = s.session.preedit();
    s.scratch_str(text)
}

/// preedit 内のフォーカス範囲 (文字単位) を out パラメータで返す。
/// Input 中はカーソル位置 (preedit 末尾, 長さ0)。
///
/// # Safety
/// `session` は有効なセッションポインタ、`from`/`len` は有効な書き込み先で
/// あること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_focused_range(
    session: *const JamoCSession,
    from: *mut u32,
    len: *mut u32,
) -> i32 {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return JAMO_ERR;
    };
    if from.is_null() || len.is_null() {
        return JAMO_ERR;
    }
    let (f, l) = s.session.focused_range();
    unsafe {
        *from = f as u32;
        *len = l as u32;
    }
    JAMO_OK
}

/// かな漢字変換を実行して Converting に遷移する。
/// `n` は文節ごとの候補数の上限の目安 (単語レベルの N-best 幅)。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_convert(session: *mut JamoCSession, n: u32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    let engine = unsafe { &*s.engine };
    let learning = engine.learning.borrow();
    match s
        .session
        .convert(&engine.dict, &engine.dict, n as usize, Some(&learning))
    {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// 文節数を返す (Input 中は 0)
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_clause_count(session: *const JamoCSession) -> i32 {
    match unsafe { session.as_ref() } {
        Some(s) => s.session.clauses().len() as i32,
        None => JAMO_ERR,
    }
}

/// 文節の読みを返す。範囲外は NULL。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_clause_yomi(
    session: *mut JamoCSession,
    clause: u32,
) -> *const c_char {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return ptr::null();
    };
    let Some(c) = s.session.clauses().get(clause as usize) else {
        return ptr::null();
    };
    let yomi = c.yomi.clone();
    s.scratch_str(yomi)
}

/// 文節の候補数を返す。範囲外は JAMO_ERR_INDEX。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_candidate_count(
    session: *const JamoCSession,
    clause: u32,
) -> i32 {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return JAMO_ERR;
    };
    match s.session.clauses().get(clause as usize) {
        Some(c) => c.candidates.len() as i32,
        None => JAMO_ERR_INDEX,
    }
}

/// 文節の候補を返す。範囲外は NULL。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_candidate(
    session: *mut JamoCSession,
    clause: u32,
    index: u32,
) -> *const c_char {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return ptr::null();
    };
    let Some(text) = s
        .session
        .clauses()
        .get(clause as usize)
        .and_then(|c| c.candidates.get(index as usize))
        .cloned()
    else {
        return ptr::null();
    };
    s.scratch_str(text)
}

/// 文節の選択中候補インデックスを返す。範囲外は JAMO_ERR_INDEX。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_selected(session: *const JamoCSession, clause: u32) -> i32 {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return JAMO_ERR;
    };
    match s.session.clauses().get(clause as usize) {
        Some(c) => c.selected as i32,
        None => JAMO_ERR_INDEX,
    }
}

/// 文節の候補を選択する
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_select(
    session: *mut JamoCSession,
    clause: u32,
    index: u32,
) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.select(clause as usize, index as usize) {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// フォーカス中の文節インデックスを返す
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_focus(session: *const JamoCSession) -> i32 {
    match unsafe { session.as_ref() } {
        Some(s) => s.session.focus() as i32,
        None => JAMO_ERR,
    }
}

/// フォーカスを移動する
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_set_focus(session: *mut JamoCSession, clause: u32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.set_focus(clause as usize) {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// フォーカス文節を delta 文字分伸縮して再変換する (+で伸ばす、-で縮める)
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_resize(
    session: *mut JamoCSession,
    delta: i32,
    n: u32,
) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    let engine = unsafe { &*s.engine };
    match s.session.resize(
        delta,
        &engine.dict,
        &engine.dict,
        n as usize,
        Some(&engine.learning.borrow()),
    ) {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// 変換を取り消して Input に戻る (かなは保持)
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_revert(session: *mut JamoCSession) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.revert() {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// 確定。確定文字列を返してセッションを初期状態に戻す。
/// Input 中はかな (+未確定ローマ字の確定分) を返す。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_commit(session: *mut JamoCSession) -> *const c_char {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return ptr::null();
    };
    let engine = unsafe { &*s.engine };
    let text = s.session.commit(Some(&mut engine.learning.borrow_mut()));
    s.scratch_str(text)
}

/// 学習データ (ユーザーデータ) をファイルへ保存する
///
/// # Safety
/// `engine` は有効なエンジンポインタ、`path` は有効なヌル終端 UTF-8
/// 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_engine_save_user_data(
    engine: *const JamoCEngine,
    path: *const c_char,
) -> i32 {
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return JAMO_ERR;
    };
    if path.is_null() {
        return JAMO_ERR;
    }
    let Ok(path) = (unsafe { CStr::from_ptr(path) }).to_str() else {
        return JAMO_ERR;
    };
    match engine.learning.borrow().save(Path::new(path)) {
        Ok(()) => JAMO_OK,
        Err(_) => JAMO_ERR,
    }
}

/// 学習データ (ユーザーデータ) をファイルから読み込む (既存の学習は置き換え)
///
/// # Safety
/// `engine` は有効なエンジンポインタ、`path` は有効なヌル終端 UTF-8
/// 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_engine_load_user_data(
    engine: *const JamoCEngine,
    path: *const c_char,
) -> i32 {
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return JAMO_ERR;
    };
    if path.is_null() {
        return JAMO_ERR;
    }
    let Ok(path) = (unsafe { CStr::from_ptr(path) }).to_str() else {
        return JAMO_ERR;
    };
    match LearningStore::load(Path::new(path)) {
        Ok(store) => {
            *engine.learning.borrow_mut() = store;
            JAMO_OK
        }
        Err(_) => JAMO_ERR,
    }
}

#[cfg(test)]
mod tests {
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
}
