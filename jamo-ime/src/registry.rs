/// COM/TSF 登録・解除
///
/// regsvr32 (管理者権限) で DLL を登録したとき、Windows が IME として認識するための
/// 3段階の登録処理 (COM CLSID, TSF Profile, TSF Category)。
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY_CLASSES_ROOT, KEY_WRITE, REG_SZ, REG_VALUE_TYPE, RegCreateKeyExW, RegDeleteTreeW,
    RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr,
    ITfInputProcessorProfileMgr,
};
use windows::core::{HRESULT, HSTRING, PCWSTR};

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

/// レジストリキーを作成して値を設定
fn reg_create_and_set(
    key_path: &str,
    value_name: PCWSTR,
    reg_type: REG_VALUE_TYPE,
    data: &[u8],
) -> Result<(), HRESULT> {
    unsafe {
        let key_path_w = HSTRING::from(key_path);
        let mut hkey = Default::default();
        let err = RegCreateKeyExW(
            HKEY_CLASSES_ROOT,
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

        let err = RegSetValueExW(hkey, value_name, Some(0), reg_type, Some(data));
        if err.0 != 0 {
            log(&format!("RegSetValueExW failed: {key_path} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

fn reg_set_string(key_path: &str, value_name: PCWSTR, data: &str) -> Result<(), HRESULT> {
    let data_w: Vec<u16> = data.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes =
        unsafe { std::slice::from_raw_parts(data_w.as_ptr() as *const u8, data_w.len() * 2) };
    reg_create_and_set(key_path, value_name, REG_SZ, bytes)
}

/// COM/TSF 登録 (管理者権限必要)
pub fn register() -> Result<(), HRESULT> {
    log("=== register start ===");

    let dll_path = get_dll_path()?;
    let clsid = guid_to_string(&CLSID_JAMO_TEXT_SERVICE);

    // 1. COM CLSID 登録
    log("step 1: COM CLSID");
    register_com_clsid(&dll_path, &clsid).map_err(|e| {
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
    log("=== unregister start ===");
    unregister_tsf_categories().ok();
    unregister_tsf_profile().ok();
    unregister_com_clsid().ok();
    log("=== unregister done ===");
    Ok(())
}

// --- Step 1: COM CLSID ---

fn register_com_clsid(dll_path: &str, clsid: &str) -> Result<(), HRESULT> {
    let key = format!("CLSID\\{clsid}\\InprocServer32");

    // DLL パス (デフォルト値)
    reg_set_string(&key, PCWSTR::null(), dll_path)?;

    // ThreadingModel
    let name: Vec<u16> = "ThreadingModel\0".encode_utf16().collect();
    reg_set_string(&key, PCWSTR(name.as_ptr()), "Apartment")?;

    Ok(())
}

fn unregister_com_clsid() -> Result<(), HRESULT> {
    let clsid = guid_to_string(&CLSID_JAMO_TEXT_SERVICE);
    let key = format!("CLSID\\{clsid}");
    let key_w = HSTRING::from(&key);
    unsafe {
        let err = RegDeleteTreeW(HKEY_CLASSES_ROOT, &key_w);
        if err.0 != 0 {
            log(&format!("RegDeleteTreeW failed: {key} err={}", err.0));
            return Err(windows::Win32::Foundation::E_FAIL);
        }
    }
    Ok(())
}

// --- Step 2: TSF Profile ---

fn register_tsf_profile(dll_path: &str) -> Result<(), HRESULT> {
    unsafe {
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| {
                    log(&format!("CoCreateInstance(ProfileMgr) failed: {e}"));
                    e.code()
                })?;

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
                true,
                0,
            )
            .map_err(|e| {
                log(&format!("RegisterProfile failed: {e}"));
                e.code()
            })?;
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

// --- Step 3: TSF Category ---

fn register_tsf_categories() -> Result<(), HRESULT> {
    unsafe {
        let cat_mgr: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER).map_err(|e| {
                log(&format!("CoCreateInstance(CategoryMgr) failed: {e}"));
                e.code()
            })?;

        for cat in &[
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
        ] {
            cat_mgr
                .RegisterCategory(&CLSID_JAMO_TEXT_SERVICE, cat, &CLSID_JAMO_TEXT_SERVICE)
                .map_err(|e| {
                    log(&format!("RegisterCategory failed: {e}"));
                    e.code()
                })?;
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
