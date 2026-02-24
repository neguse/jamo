/// COM/TSF 登録・解除
///
/// regsvr32 で DLL を登録したとき、Windows が IME として認識するための
/// 3段階の登録処理 (COM CLSID, TSF Profile, TSF Category)。
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY_CLASSES_ROOT, KEY_WRITE, REG_SZ, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr,
    ITfInputProcessorProfileMgr,
};
use windows::core::{HRESULT, HSTRING, Interface as _, w};

use crate::globals::{CLSID_JAMO_TEXT_SERVICE, GUID_JAMO_PROFILE, get_dll_instance};

const LANGID_JAPANESE: u16 = 0x0411;

fn get_dll_path() -> Result<String, HRESULT> {
    let hmodule = get_dll_instance();
    let mut path = [0u16; MAX_PATH as usize];
    let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut path) } as usize;
    if len == 0 {
        return Err(windows::Win32::Foundation::E_FAIL);
    }
    Ok(String::from_utf16_lossy(&path[..len]))
}

/// COM/TSF 登録
pub fn register() -> Result<(), HRESULT> {
    let dll_path = get_dll_path()?;

    // 1. COM CLSID 登録
    register_com_clsid(&dll_path)?;

    // 2. TSF Profile 登録
    register_tsf_profile(&dll_path)?;

    // 3. TSF Category 登録
    register_tsf_categories()?;

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

fn register_com_clsid(dll_path: &str) -> Result<(), HRESULT> {
    let clsid_str = format!("{{{:?}}}", CLSID_JAMO_TEXT_SERVICE);
    let key_path = format!("CLSID\\{clsid_str}\\InprocServer32");
    let key_path_w = HSTRING::from(&key_path);

    unsafe {
        let mut hkey = Default::default();
        RegCreateKeyExW(
            HKEY_CLASSES_ROOT,
            &key_path_w,
            0,
            None,
            Default::default(),
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
        .map_err(|e| e.code())?;

        // DLL パス (デフォルト値)
        let dll_path_w: Vec<u16> = dll_path.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey,
            None,
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                dll_path_w.as_ptr() as *const u8,
                dll_path_w.len() * 2,
            )),
        )
        .map_err(|e| e.code())?;

        // ThreadingModel
        let threading = w!("Apartment");
        let threading_bytes: &[u8] =
            std::slice::from_raw_parts(threading.as_ptr() as *const u8, (threading.len() + 1) * 2);
        RegSetValueExW(hkey, w!("ThreadingModel"), 0, REG_SZ, Some(threading_bytes))
            .map_err(|e| e.code())?;
    }
    Ok(())
}

fn unregister_com_clsid() -> Result<(), HRESULT> {
    let clsid_str = format!("{{{:?}}}", CLSID_JAMO_TEXT_SERVICE);
    let key_path = format!("CLSID\\{clsid_str}");
    let key_path_w = HSTRING::from(&key_path);

    unsafe {
        RegDeleteTreeW(HKEY_CLASSES_ROOT, &key_path_w).map_err(|e| e.code())?;
    }
    Ok(())
}

fn register_tsf_profile(dll_path: &str) -> Result<(), HRESULT> {
    unsafe {
        let profile_mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| e.code())?;

        let dll_path_w = HSTRING::from(dll_path);

        profile_mgr
            .RegisterProfile(
                &CLSID_JAMO_TEXT_SERVICE,
                LANGID_JAPANESE,
                &GUID_JAMO_PROFILE,
                &HSTRING::from("Jamo"),
                &dll_path_w,
                0, // icon index
                std::ptr::null(),
                0,
                0,
                1, // enabled
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
