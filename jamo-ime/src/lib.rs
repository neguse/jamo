pub mod composition;
pub mod globals;

#[cfg(windows)]
pub mod candidate_ui;
#[cfg(windows)]
pub mod class_factory;
#[cfg(windows)]
pub mod display_attribute;
#[cfg(windows)]
pub mod edit_session;
#[cfg(windows)]
pub mod engine;
#[cfg(windows)]
pub mod registry;
#[cfg(windows)]
pub mod text_service;

// --- COM DLL エントリポイント ---

#[cfg(windows)]
mod dll {
    use std::ffi::c_void;
    use std::sync::atomic::Ordering;

    use windows::Win32::Foundation::{HINSTANCE, S_FALSE, S_OK};
    use windows::Win32::System::Com::IClassFactory;
    use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
    use windows::core::{BOOL, GUID, HRESULT, Interface as _};

    use crate::class_factory::JamoClassFactory;
    use crate::globals::{CLSID_JAMO_TEXT_SERVICE, SERVER_LOCK_COUNT, set_dll_instance};

    /// DLL エントリポイント
    #[unsafe(no_mangle)]
    extern "system" fn DllMain(inst: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
        if reason == DLL_PROCESS_ATTACH {
            set_dll_instance(inst.into());
        }
        BOOL(1)
    }

    /// COM オブジェクト生成
    #[unsafe(no_mangle)]
    extern "system" fn DllGetClassObject(
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        unsafe {
            if ppv.is_null() || rclsid.is_null() || riid.is_null() {
                return windows::Win32::Foundation::E_INVALIDARG;
            }
            *ppv = std::ptr::null_mut();

            let rclsid = &*rclsid;
            let riid = &*riid;

            if *rclsid != CLSID_JAMO_TEXT_SERVICE {
                return windows::Win32::Foundation::CLASS_E_CLASSNOTAVAILABLE;
            }

            let factory: IClassFactory = JamoClassFactory.into();
            let unknown: windows::core::IUnknown = factory.cast().unwrap();
            unknown
                .QueryInterface(riid, ppv)
                .ok()
                .map_or_else(|e| e.into(), |()| S_OK)
        }
    }

    /// DLL アンロード可能か
    #[unsafe(no_mangle)]
    extern "system" fn DllCanUnloadNow() -> HRESULT {
        if SERVER_LOCK_COUNT.load(Ordering::SeqCst) == 0 {
            S_OK
        } else {
            S_FALSE
        }
    }

    /// COM サーバー登録
    #[unsafe(no_mangle)]
    extern "system" fn DllRegisterServer() -> HRESULT {
        match crate::registry::register() {
            Ok(()) => S_OK,
            Err(e) => e,
        }
    }

    /// COM サーバー登録解除
    #[unsafe(no_mangle)]
    extern "system" fn DllUnregisterServer() -> HRESULT {
        match crate::registry::unregister() {
            Ok(()) => S_OK,
            Err(e) => e,
        }
    }
}
