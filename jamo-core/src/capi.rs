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

use crate::bindict::BinDict;
use crate::learning::LearningStore;
use crate::session::{Session, SessionError, SessionState, Transform};

#[cfg(test)]
mod tests;

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

/// jamo_session_transform の種別: ひらがな
pub const JAMO_TRANSFORM_HIRAGANA: i32 = 0;
/// jamo_session_transform の種別: カタカナ
pub const JAMO_TRANSFORM_KATAKANA: i32 = 1;
/// jamo_session_transform の種別: 半角英字 (打鍵ローマ字)
pub const JAMO_TRANSFORM_HALF_ALPHA: i32 = 2;
/// jamo_session_transform の種別: 全角英字
pub const JAMO_TRANSFORM_WIDE_ALPHA: i32 = 3;

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
    dict: BinDict,
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
    match BinDict::open(Path::new(path)) {
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

/// カーソル直後のかなを1文字消す (Delete)。未確定ローマ字は先に確定する。
///
/// 返値: 1 = 消した、0 = 消すものがない、負 = エラー。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_delete_forward(session: *mut JamoCSession) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.delete_forward() {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => error_code(e),
    }
}

/// preedit 内のカーソル位置 (文字単位)。Input 中のみ意味を持つ。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_cursor(session: *const JamoCSession) -> i32 {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return JAMO_ERR;
    };
    s.session.cursor() as i32
}

/// カーソルを文字単位で移動する。未確定ローマ字は先に確定する。
///
/// 返値: 1 = 動いた、0 = 端で動けない、負 = エラー。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_move_cursor(session: *mut JamoCSession, delta: i32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.move_cursor(delta) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => error_code(e),
    }
}

/// カーソルを文字インデックスへ移動する (範囲外は端に丸める)。
/// 未確定ローマ字は先に確定する。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_set_cursor(session: *mut JamoCSession, pos: u32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.set_cursor(pos as usize) {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// 一時英字モードか (1 = 英字モード、0 = かな、負 = エラー)。
/// 大文字の入力で自動的に on になり、確定/リセットで off になる。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_is_alpha(session: *const JamoCSession) -> i32 {
    let Some(s) = (unsafe { session.as_ref() }) else {
        return JAMO_ERR;
    };
    s.session.is_alpha() as i32
}

/// 一時英字モードを明示的に切り替える (0 = かな、それ以外 = 英字)
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_set_alpha(session: *mut JamoCSession, on: i32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    match s.session.set_alpha(on != 0) {
        Ok(()) => JAMO_OK,
        Err(e) => error_code(e),
    }
}

/// preedit 全体を文字種変換 (JAMO_TRANSFORM_*) した 1 文節の変換状態にする。
/// 同じ種別を繰り返すと候補 (大小文字) を巡回する。変換中なら読み全体に適用する。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_transform(session: *mut JamoCSession, kind: i32) -> i32 {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return JAMO_ERR;
    };
    let kind = match kind {
        JAMO_TRANSFORM_HIRAGANA => Transform::Hiragana,
        JAMO_TRANSFORM_KATAKANA => Transform::Katakana,
        JAMO_TRANSFORM_HALF_ALPHA => Transform::HalfAlpha,
        JAMO_TRANSFORM_WIDE_ALPHA => Transform::WideAlpha,
        _ => return JAMO_ERR,
    };
    match s.session.transform(kind) {
        Ok(()) => JAMO_OK,
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

/// 部分確定。フォーカス文節までの確定文字列を返し、残りの文節は変換中のまま
/// 残す (フォーカスが最後の文節なら `jamo_session_commit` と同じ)。
///
/// # Safety
/// `session` は有効なセッションポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_session_commit_prefix(session: *mut JamoCSession) -> *const c_char {
    let Some(s) = (unsafe { session.as_mut() }) else {
        return ptr::null();
    };
    let engine = unsafe { &*s.engine };
    let text = s
        .session
        .commit_prefix(Some(&mut engine.learning.borrow_mut()));
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
