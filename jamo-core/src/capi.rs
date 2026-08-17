//! C API バインディング
//!
//! `#[unsafe(no_mangle)] extern "C"` で公開する。
//! cbindgen でヘッダを自動生成。

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

use crate::ipadic::IpaDict;
use crate::romaji::{BackspaceResult, RomajiConverter, RomajiResult};

/// 不透明なエンジンハンドル
pub struct JamoCEngine {
    dict: IpaDict,
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
        Ok(dict) => Box::into_raw(Box::new(JamoCEngine { dict })),
        Err(_) => ptr::null_mut(),
    }
}

/// エンジンを解放
///
/// # Safety
/// `engine` は `jamo_engine_open` が返した有効なポインタ、または NULL であること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_engine_free(engine: *mut JamoCEngine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// かな漢字変換
///
/// JSON 文字列を返す。呼び出し側は `jamo_string_free` で解放すること。
/// 失敗時は NULL を返す。
///
/// # Safety
/// `engine` は有効なエンジンポインタ、`input` は有効なヌル終端 UTF-8 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_convert(
    engine: *const JamoCEngine,
    input: *const c_char,
    n: u32,
) -> *mut c_char {
    if engine.is_null() || input.is_null() {
        return ptr::null_mut();
    }
    let engine = unsafe { &*engine };
    let input = match unsafe { CStr::from_ptr(input) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let result = crate::convert_nbest(input, &engine.dict, &engine.dict, n as usize);

    match CString::new(convert_result_to_json(&result)) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// ConvertResult を C API 共通の JSON ({"result":[...],"boundaries":[...]}) にする
/// (serde_json がない場合は手動)
fn convert_result_to_json(result: &crate::viterbi::ConvertResult) -> String {
    let mut json = String::from("{\"result\":[");
    for (i, clause) in result.clauses.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"yomi\":\"");
        json.push_str(&escape_json(&clause.yomi));
        json.push_str("\",\"candidates\":[");
        for (j, cand) in clause.candidates.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            json.push('"');
            json.push_str(&escape_json(cand));
            json.push('"');
        }
        json.push_str("]}");
    }
    json.push_str("],\"boundaries\":[");
    let mut pos = 0;
    for (i, clause) in result.clauses.iter().enumerate() {
        pos += clause.yomi.len();
        if i > 0 {
            json.push(',');
        }
        json.push_str(&pos.to_string());
    }
    json.push_str("]}");
    json
}

/// 文節伸縮
///
/// `boundaries` は `jamo_convert` が返す boundaries と同じ、カンマ区切りの
/// 文節境界バイト位置。`clause_idx` の文節を `delta` 文字分伸縮 (+で伸ばす、
/// -で縮める) し、新しい境界で再変換した結果を `jamo_convert` と同じ JSON で返す。
/// 不正な操作・引数の場合は NULL を返す。
///
/// # Safety
/// `engine` は有効なエンジンポインタ、`input` と `boundaries` は有効な
/// ヌル終端 UTF-8 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_resize(
    engine: *const JamoCEngine,
    input: *const c_char,
    boundaries: *const c_char,
    clause_idx: u32,
    delta: i32,
    n: u32,
) -> *mut c_char {
    if engine.is_null() || input.is_null() || boundaries.is_null() {
        return ptr::null_mut();
    }
    let engine = unsafe { &*engine };
    let input = match unsafe { CStr::from_ptr(input) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let boundaries_str = match unsafe { CStr::from_ptr(boundaries) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let parsed: Result<Vec<usize>, _> = boundaries_str
        .split(',')
        .map(|s| s.trim().parse::<usize>())
        .collect();
    let parsed = match parsed {
        Ok(v) => v,
        Err(_) => return ptr::null_mut(),
    };

    let new_boundaries = match crate::resize_boundaries(input, &parsed, clause_idx as usize, delta)
    {
        Some(b) => b,
        None => return ptr::null_mut(),
    };
    let result = match crate::try_convert_with_boundaries(
        input,
        &new_boundaries,
        &engine.dict,
        &engine.dict,
        n as usize,
    ) {
        Ok(r) => r,
        Err(_) => return ptr::null_mut(),
    };

    match CString::new(convert_result_to_json(&result)) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// 予測変換
///
/// JSON 文字列を返す。呼び出し側は `jamo_string_free` で解放すること。
///
/// # Safety
/// `engine` は有効なエンジンポインタ、`prefix` は有効なヌル終端 UTF-8 文字列を指すこと。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_predict(
    engine: *const JamoCEngine,
    prefix: *const c_char,
    limit: u32,
) -> *mut c_char {
    if engine.is_null() || prefix.is_null() {
        return ptr::null_mut();
    }
    let engine = unsafe { &*engine };
    let prefix = match unsafe { CStr::from_ptr(prefix) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let entries = engine.dict.prefix_search(prefix, limit as usize);
    let mut seen = std::collections::HashSet::new();

    let mut json = String::from("{\"candidates\":[");
    let mut first = true;
    for entry in &entries {
        if !seen.insert(&entry.surface) {
            continue;
        }
        if !first {
            json.push(',');
        }
        json.push_str("{\"surface\":\"");
        json.push_str(&escape_json(&entry.surface));
        json.push_str("\"}");
        first = false;
    }
    json.push_str("]}");

    match CString::new(json) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// C API が返した文字列を解放
///
/// # Safety
/// `s` は `jamo_convert` / `jamo_predict` が返したポインタ、または NULL であること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// 不透明なローマ字変換器ハンドル
pub struct JamoCRomaji {
    conv: RomajiConverter,
}

/// jamo_romaji_backspace の返値: 未確定ローマ字を1文字消した
pub const JAMO_BACKSPACE_PENDING: i32 = 0;
/// jamo_romaji_backspace の返値: 未確定ローマ字が空。呼び出し側がひらがなを消すべき
pub const JAMO_BACKSPACE_HIRAGANA: i32 = 1;
/// jamo_romaji_backspace の返値: 消すものがない
pub const JAMO_BACKSPACE_EMPTY: i32 = 2;

/// ローマ字変換器を生成
///
/// 呼び出し側は使用後に `jamo_romaji_free` で解放すること。
#[unsafe(no_mangle)]
pub extern "C" fn jamo_romaji_new() -> *mut JamoCRomaji {
    Box::into_raw(Box::new(JamoCRomaji {
        conv: RomajiConverter::new(),
    }))
}

/// ローマ字変換器を解放
///
/// # Safety
/// `romaji` は `jamo_romaji_new` が返した有効なポインタ、または NULL であること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_romaji_free(romaji: *mut JamoCRomaji) {
    if !romaji.is_null() {
        drop(unsafe { Box::from_raw(romaji) });
    }
}

/// 1文字入力し、確定したひらがな (未確定なら空文字列) を返す
///
/// `ch` は Unicode スカラ値。返値は `jamo_string_free` で解放すること。
/// 不正な引数の場合は NULL を返す。
///
/// # Safety
/// `romaji` は `jamo_romaji_new` が返した有効なポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_romaji_feed(romaji: *mut JamoCRomaji, ch: u32) -> *mut c_char {
    if romaji.is_null() {
        return ptr::null_mut();
    }
    let ch = match char::from_u32(ch) {
        Some(c) => c,
        None => return ptr::null_mut(),
    };
    let out = match unsafe { &mut (*romaji).conv }.feed(ch) {
        RomajiResult::Emit(s) => s,
        RomajiResult::Pending => String::new(),
    };
    match CString::new(out) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Backspace 処理。JAMO_BACKSPACE_* のいずれかを返す
///
/// # Safety
/// `romaji` は `jamo_romaji_new` が返した有効なポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_romaji_backspace(romaji: *mut JamoCRomaji) -> i32 {
    if romaji.is_null() {
        return JAMO_BACKSPACE_EMPTY;
    }
    match unsafe { &mut (*romaji).conv }.backspace() {
        BackspaceResult::DeletedPending => JAMO_BACKSPACE_PENDING,
        BackspaceResult::DeleteHiragana => JAMO_BACKSPACE_HIRAGANA,
        BackspaceResult::Empty => JAMO_BACKSPACE_EMPTY,
    }
}

/// 未確定ローマ字を強制確定し、確定分 ("n" → "ん"、なければ空文字列) を返す
///
/// 返値は `jamo_string_free` で解放すること。
///
/// # Safety
/// `romaji` は `jamo_romaji_new` が返した有効なポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_romaji_flush(romaji: *mut JamoCRomaji) -> *mut c_char {
    if romaji.is_null() {
        return ptr::null_mut();
    }
    match CString::new(unsafe { &mut (*romaji).conv }.flush()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// 現在の未確定ローマ字のコピーを返す
///
/// 返値は `jamo_string_free` で解放すること。
///
/// # Safety
/// `romaji` は `jamo_romaji_new` が返した有効なポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_romaji_pending(romaji: *const JamoCRomaji) -> *mut c_char {
    if romaji.is_null() {
        return ptr::null_mut();
    }
    match CString::new(unsafe { &(*romaji).conv }.pending().to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// 未確定ローマ字を破棄する
///
/// # Safety
/// `romaji` は `jamo_romaji_new` が返した有効なポインタであること。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jamo_romaji_reset(romaji: *mut JamoCRomaji) {
    if !romaji.is_null() {
        unsafe { &mut (*romaji).conv }.reset();
    }
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn cstr(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    fn take_string(p: *mut c_char) -> String {
        assert!(!p.is_null());
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        unsafe { jamo_string_free(p) };
        s
    }

    #[test]
    fn test_romaji_feed_basic() {
        let r = jamo_romaji_new();
        unsafe {
            assert_eq!(take_string(jamo_romaji_feed(r, 'k' as u32)), "");
            assert_eq!(take_string(jamo_romaji_feed(r, 'y' as u32)), "");
            assert_eq!(take_string(jamo_romaji_feed(r, 'o' as u32)), "きょ");
            assert_eq!(take_string(jamo_romaji_feed(r, 'u' as u32)), "う");
            jamo_romaji_free(r);
        }
    }

    #[test]
    fn test_romaji_backspace_and_flush() {
        let r = jamo_romaji_new();
        unsafe {
            take_string(jamo_romaji_feed(r, 'k' as u32));
            assert_eq!(jamo_romaji_backspace(r), JAMO_BACKSPACE_PENDING);
            assert_eq!(jamo_romaji_backspace(r), JAMO_BACKSPACE_HIRAGANA);
            take_string(jamo_romaji_feed(r, 'n' as u32));
            assert_eq!(take_string(jamo_romaji_flush(r)), "ん");
            jamo_romaji_free(r);
        }
    }

    #[test]
    fn test_romaji_pending_and_reset() {
        let r = jamo_romaji_new();
        unsafe {
            take_string(jamo_romaji_feed(r, 'k' as u32));
            assert_eq!(take_string(jamo_romaji_pending(r)), "k");
            jamo_romaji_reset(r);
            assert_eq!(take_string(jamo_romaji_pending(r)), "");
            jamo_romaji_free(r);
        }
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
    fn test_capi_convert_and_resize() {
        let Some(e) = open_test_engine() else { return };
        unsafe {
            let input = cstr("きょうはいいてんきですね");
            let json = take_string(jamo_convert(e, input.as_ptr(), 1));
            assert!(json.contains("\"result\":["));
            assert!(json.contains("\"boundaries\":["));

            // 返ってきた boundaries で先頭文節を1文字伸ばす
            let b = json
                .rsplit("\"boundaries\":[")
                .next()
                .unwrap()
                .trim_end_matches("]}");
            let boundaries = cstr(b);
            let resized = jamo_resize(e, input.as_ptr(), boundaries.as_ptr(), 0, 1, 1);
            assert!(!resized.is_null());
            let rj = take_string(resized);
            assert!(rj.contains("\"result\":["));
            assert!(rj.contains("\"boundaries\":["));

            jamo_engine_free(e);
        }
    }

    #[test]
    fn test_capi_resize_rejects_bad_input() {
        let Some(e) = open_test_engine() else { return };
        unsafe {
            let input = cstr("きょうは");
            let bad = cstr("not,numbers");
            assert!(jamo_resize(e, input.as_ptr(), bad.as_ptr(), 0, 1, 1).is_null());
            jamo_engine_free(e);
        }
    }
}
