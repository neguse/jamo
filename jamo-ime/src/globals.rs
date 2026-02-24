/// グローバル定数・状態

#[cfg(windows)]
use std::sync::atomic::AtomicU32;

#[cfg(windows)]
use windows::core::GUID;

// --- GUID 定義 ---

/// テキストサービスの CLSID
#[cfg(windows)]
pub const CLSID_JAMO_TEXT_SERVICE: GUID =
    GUID::from_u128(0x8a3f_c252_7e14_4b8a_9f6d_1c2e_5a3b_7d90);

/// 言語プロファイル GUID
#[cfg(windows)]
pub const GUID_JAMO_PROFILE: GUID = GUID::from_u128(0x9b4f_d363_8f25_5c9b_a07e_2d3f_6b4c_8ea1);

/// 半角/全角切替の PreservedKey GUID
#[cfg(windows)]
pub const GUID_PRESERVED_KEY_ONOFF: GUID =
    GUID::from_u128(0xac50_e474_9036_6dac_b18f_3e40_7c5d_9fb2);

/// Alt+` 切替の PreservedKey GUID (HHKB 等)
#[cfg(windows)]
pub const GUID_PRESERVED_KEY_ALT_BACKQUOTE: GUID =
    GUID::from_u128(0xbd61_f585_a147_7ebd_c290_4f51_8d6e_a0c4);

/// DisplayAttribute: 入力中 (点線下線)
#[cfg(windows)]
pub const GUID_DA_INPUT: GUID = GUID::from_u128(0xbd61_f585_a147_7ebd_c290_4f51_8d6e_a0c3);

/// DisplayAttribute: 変換済み (実線下線)
#[cfg(windows)]
pub const GUID_DA_CONVERTED: GUID = GUID::from_u128(0xce72_0696_b258_8fce_d3a1_5062_9e7f_b1d4);

/// DisplayAttribute: フォーカス文節 (太実線下線)
#[cfg(windows)]
pub const GUID_DA_FOCUSED: GUID = GUID::from_u128(0xdf83_17a7_c369_90df_e4b2_6173_af80_c2e5);

// --- グローバル状態 ---

/// DllMain で保存する DLL の HMODULE (usize として保存、HMODULE は !Send+!Sync)
#[cfg(windows)]
pub static DLL_INSTANCE_RAW: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(windows)]
pub fn set_dll_instance(h: windows::Win32::Foundation::HMODULE) {
    DLL_INSTANCE_RAW.store(h.0 as usize, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(windows)]
pub fn get_dll_instance() -> windows::Win32::Foundation::HMODULE {
    windows::Win32::Foundation::HMODULE(
        DLL_INSTANCE_RAW.load(std::sync::atomic::Ordering::SeqCst) as *mut _
    )
}

/// COM サーバーロックカウント (DllCanUnloadNow 用)
#[cfg(windows)]
pub static SERVER_LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

/// COM QueryInterface (vtable 直接呼び出し)
///
/// windows-rs 0.62 では IUnknown に query/QueryInterface メソッドがないため、
/// vtable 経由で直接呼び出す。
#[cfg(windows)]
pub unsafe fn com_query_interface(
    obj: *mut std::ffi::c_void,
    iid: *const GUID,
    ppv: *mut *mut std::ffi::c_void,
) -> windows::core::HRESULT {
    type QIFn = unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const windows::core::GUID,
        *mut *mut std::ffi::c_void,
    ) -> windows::core::HRESULT;
    unsafe {
        let vtbl = *(obj as *const *const QIFn);
        (*vtbl)(obj, iid, ppv)
    }
}
