//! C API バインディング
//!
//! `#[unsafe(no_mangle)] extern "C"` で公開する。
//! cbindgen でヘッダを自動生成。

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

use crate::ipadic::IpaDict;

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

    // JSON 構築 (serde_json がない場合は手動)
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

    match CString::new(json) {
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
