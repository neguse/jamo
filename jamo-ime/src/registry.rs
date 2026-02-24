/// COM/TSF 登録・解除
///
/// regsvr32 で DLL を登録したとき、Windows が IME として認識するための
/// 3段階の登録処理 (COM CLSID, TSF Profile, TSF Category)。
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr,
    ITfInputProcessorProfileMgr,
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

/// COM/TSF 登録
pub fn register() -> Result<(), HRESULT> {
    log("=== register start ===");

    let dll_path = get_dll_path()?;

    // 1. COM CLSID 登録
    log("step 1: COM CLSID");
    register_com_clsid(&dll_path).map_err(|e| {
        log(&format!("FAILED: COM CLSID: {e:?}"));
        e
    })?;
    log("step 1: OK");

    // 2. TSF Profile 登録
    log("step 2: TSF Profile");
    register_tsf_profile(&dll_path).map_err(|e| {
        log(&format!("FAILED: TSF Profile: {e:?}"));
        e
    })?;
    log("step 2: OK");

    // 3. TSF Category 登録
    log("step 3: TSF Category");
    register_tsf_categories().map_err(|e| {
        log(&format!("FAILED: TSF Category: {e:?}"));
        e
    })?;
    log("step 3: OK");

    log("=== register done ===");
    Ok(())
}

/// COM/TSF 登録解除
pub fn unregister() -> Result<(), HRESULT> {
    // 逆順で解除
    unregister_tsf_categories().ok();
    unregister_tsf_profile().ok();
    unregister_com_clsid().ok();
    Ok(())
}

/// Per-user COM 登録のルートキー
/// HKCU\Software\Classes は HKCR のユーザー側オーバーレイ。管理者権限不要。
const REG_ROOT: &str = "Software\\Classes";

fn register_com_clsid(dll_path: &str) -> Result<(), HRESULT> {
    let clsid_str = format!("{{{:?}}}", CLSID_JAMO_TEXT_SERVICE);
    let key_path = format!("{REG_ROOT}\\CLSID\\{clsid_str}\\InprocServer32");
    let key_path_w = HSTRING::from(&key_path);

    unsafe {
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
            return Err(windows::Win32::Foundation::E_FAIL);
        }

        // DLL パス (デフォルト値)
        let dll_path_w: Vec<u16> = dll_path.encode_utf16().chain(std::iter::once(0)).collect();
        let err = RegSetValueExW(
            hkey,
            None,
            Some(0),
            REG_SZ,
            Some(std::slice::from_raw_parts(
                dll_path_w.as_ptr() as *const u8,
                dll_path_w.len() * 2,
            )),
        );
        if err.0 != 0 {
            return Err(windows::Win32::Foundation::E_FAIL);
        }

        // ThreadingModel
        let threading = w!("Apartment");
        let threading_bytes: &[u8] =
            std::slice::from_raw_parts(threading.as_ptr() as *const u8, (threading.len() + 1) * 2);
        let err = RegSetValueExW(
            hkey,
            w!("ThreadingModel"),
            Some(0),
            REG_SZ,
            Some(threading_bytes),
        );
        if err.0 != 0 {
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

fn unregister_com_clsid() -> Result<(), HRESULT> {
    let clsid_str = format!("{{{:?}}}", CLSID_JAMO_TEXT_SERVICE);
    let key_path = format!("{REG_ROOT}\\CLSID\\{clsid_str}");
    let key_path_w = HSTRING::from(&key_path);

    unsafe {
        let err = RegDeleteTreeW(HKEY_CURRENT_USER, &key_path_w);
        if err.0 != 0 {
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

fn register_tsf_profile(dll_path: &str) -> Result<(), HRESULT> {
    unsafe {
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| e.code())?;

        let desc: Vec<u16> = "Jamo".encode_utf16().collect();
        let dll_path_u16: Vec<u16> = dll_path.encode_utf16().collect();

        profile_mgr
            .RegisterProfile(
                &CLSID_JAMO_TEXT_SERVICE,
                LANGID_JAPANESE,
                &GUID_JAMO_PROFILE,
                &desc,
                &dll_path_u16,
                0, // icon index
                HKL::default(),
                0,
                true, // enabled
                0,
            )
            .map_err(|e| e.code())?;
    }
    Ok(())
}

fn unregister_tsf_profile() -> Result<(), HRESULT> {
    unsafe {
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| e.code())?;

        profile_mgr
            .UnregisterProfile(
                &CLSID_JAMO_TEXT_SERVICE,
                LANGID_JAPANESE,
                &GUID_JAMO_PROFILE,
                0,
            )
            .map_err(|e| e.code())?;
    }
    Ok(())
}

fn register_tsf_categories() -> Result<(), HRESULT> {
    unsafe {
        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| e.code())?;

        for cat in &[
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
        ] {
            cat_mgr
                .RegisterCategory(&CLSID_JAMO_TEXT_SERVICE, cat, &CLSID_JAMO_TEXT_SERVICE)
                .map_err(|e| e.code())?;
        }
    }
    Ok(())
}

fn unregister_tsf_categories() -> Result<(), HRESULT> {
    unsafe {
        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| e.code())?;

        for cat in &[
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
        ] {
            cat_mgr
                .UnregisterCategory(&CLSID_JAMO_TEXT_SERVICE, cat, &CLSID_JAMO_TEXT_SERVICE)
                .ok();
        }
    }
    Ok(())
}
