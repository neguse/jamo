/// COM/TSF 登録・解除 (Per-user, 管理者権限不要)
///
/// regsvr32 で DLL を登録したとき、Windows が IME として認識するための登録処理。
/// 全て HKCU に書き込むため管理者権限不要。
///
/// 登録先:
/// - COM CLSID: HKCU\Software\Classes\CLSID\{CLSID}\InprocServer32
/// - TSF Profile: HKCU\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x00000411\{Profile}
/// - TSF Category: HKCU\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\Category\Category\{CatGUID}
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_WRITE, REG_DWORD, REG_SZ, RegCreateKeyExW, RegDeleteTreeW,
    RegSetValueExW,
};
use windows::Win32::UI::TextServices::{
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER, GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
};
use windows::core::{HRESULT, HSTRING, w};

use crate::globals::{CLSID_JAMO_TEXT_SERVICE, GUID_JAMO_PROFILE, get_dll_instance};

const LANGID_JAPANESE: u16 = 0x0411;

/// デバッグログをファイルに出力 (%LOCALAPPDATA%\Jamo\register.log)
fn log(msg: &str) {
    use std::io::Write;
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let dir = std::path::PathBuf::from(local_app_data).join("Jamo");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("register.log"))
        {
            let _ = writeln!(f, "{msg}");
        }
    }
}

fn guid_to_string(guid: &windows::core::GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

fn get_dll_path() -> Result<String, HRESULT> {
    let hmodule = get_dll_instance();
    let mut path = [0u16; MAX_PATH as usize];
    let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut path) } as usize;
    if len == 0 {
        log("ERROR: GetModuleFileNameW returned 0");
        return Err(windows::Win32::Foundation::E_FAIL);
    }
    let dll_path = String::from_utf16_lossy(&path[..len]);
    log(&format!("DLL path: {dll_path}"));
    Ok(dll_path)
}

/// レジストリキーを作成して文字列値を設定するヘルパー
fn reg_set_string(key_path: &str, value_name: Option<&HSTRING>, data: &str) -> Result<(), HRESULT> {
    unsafe {
        let key_path_w = HSTRING::from(key_path);
        let mut hkey = Default::default();
        let err = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &key_path_w,
            Some(0),
            None,
            Default::default(),
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if err.0 != 0 {
            log(&format!("RegCreateKeyExW failed: {key_path} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }

        let data_w: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
        let err = RegSetValueExW(
            hkey,
            value_name,
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                data_w.as_ptr() as *const u8,
                data_w.len() * 2,
            )),
        );
        if err.0 != 0 {
            log(&format!("RegSetValueExW failed: {key_path} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

/// レジストリキーを作成して DWORD 値を設定するヘルパー
fn reg_set_dword(key_path: &str, value_name: &HSTRING, data: u32) -> Result<(), HRESULT> {
    unsafe {
        let key_path_w = HSTRING::from(key_path);
        let mut hkey = Default::default();
        let err = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &key_path_w,
            Some(0),
            None,
            Default::default(),
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if err.0 != 0 {
            log(&format!("RegCreateKeyExW failed: {key_path} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }

        let bytes = data.to_le_bytes();
        let err = RegSetValueExW(hkey, value_name, Some(0), REG_DWORD, Some(&bytes));
        if err.0 != 0 {
            log(&format!("RegSetValueExW failed: {key_path} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

/// COM/TSF 登録 (全て HKCU、管理者権限不要)
pub fn register() -> Result<(), HRESULT> {
    log("=== register start ===");

    let dll_path = get_dll_path()?;
    let clsid = guid_to_string(&CLSID_JAMO_TEXT_SERVICE);
    let profile = guid_to_string(&GUID_JAMO_PROFILE);

    // 1. COM CLSID 登録
    log("step 1: COM CLSID");
    register_com_clsid(&dll_path, &clsid)?;
    log("step 1: OK");

    // 2. TSF Profile 登録 (直接レジストリ書き込み)
    log("step 2: TSF Profile");
    register_tsf_profile(&dll_path, &clsid, &profile)?;
    log("step 2: OK");

    // 3. TSF Category 登録 (直接レジストリ書き込み)
    log("step 3: TSF Category");
    register_tsf_categories(&clsid)?;
    log("step 3: OK");

    log("=== register done ===");
    Ok(())
}

/// COM/TSF 登録解除
pub fn unregister() -> Result<(), HRESULT> {
    log("=== unregister start ===");

    let clsid = guid_to_string(&CLSID_JAMO_TEXT_SERVICE);

    // COM CLSID 削除
    let key = format!("Software\\Classes\\CLSID\\{clsid}");
    let _ = reg_delete_tree(&key);

    // TSF TIP 削除
    let key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{clsid}");
    let _ = reg_delete_tree(&key);

    log("=== unregister done ===");
    Ok(())
}

fn reg_delete_tree(key_path: &str) -> Result<(), HRESULT> {
    let key_path_w = HSTRING::from(key_path);
    unsafe {
        let err = RegDeleteTreeW(HKEY_CURRENT_USER, &key_path_w);
        if err.0 != 0 {
            log(&format!("RegDeleteTreeW: {key_path} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

// --- Step 1: COM CLSID ---

fn register_com_clsid(dll_path: &str, clsid: &str) -> Result<(), HRESULT> {
    let key = format!("Software\\Classes\\CLSID\\{clsid}\\InprocServer32");

    // DLL パス (デフォルト値)
    reg_set_string(&key, None, dll_path)?;

    // ThreadingModel
    reg_set_string(&key, Some(&HSTRING::from("ThreadingModel")), "Apartment")?;

    Ok(())
}

// --- Step 2: TSF Profile (直接レジストリ) ---
// HKCU\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x00000411\{Profile}

fn register_tsf_profile(dll_path: &str, clsid: &str, profile: &str) -> Result<(), HRESULT> {
    let base = format!(
        "SOFTWARE\\Microsoft\\CTF\\TIP\\{clsid}\\LanguageProfile\\0x{LANGID_JAPANESE:08X}\\{profile}"
    );

    // Description
    reg_set_string(&base, Some(&HSTRING::from("Description")), "Jamo")?;

    // IconFile (DLL パス)
    reg_set_string(&base, Some(&HSTRING::from("IconFile")), dll_path)?;

    // IconIndex
    reg_set_dword(&base, &HSTRING::from("IconIndex"), 0)?;

    // Enable
    reg_set_dword(&base, &HSTRING::from("Enable"), 1)?;

    Ok(())
}

// --- Step 3: TSF Category (直接レジストリ) ---
// HKCU\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\Category\Category\{CatGUID}
//   値名: {CLSID} = ""

fn register_tsf_categories(clsid: &str) -> Result<(), HRESULT> {
    let categories = [
        GUID_TFCAT_TIP_KEYBOARD,
        GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
        GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    ];

    for cat in &categories {
        let cat_str = guid_to_string(cat);
        let key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{clsid}\\Category\\Category\\{cat_str}");
        reg_set_string(&key, Some(&HSTRING::from(clsid)), "")?;
    }

    // Item カテゴリ (CLSID → Category のマッピング)
    for cat in &categories {
        let cat_str = guid_to_string(cat);
        let key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{clsid}\\Category\\Item\\{clsid}");
        reg_set_string(&key, Some(&HSTRING::from(&cat_str)), "")?;
    }

    Ok(())
}
