/// IClassFactory 実装
///
/// COM オブジェクト生成ファクトリ。DllGetClassObject から返却される。
use std::ffi::c_void;
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Com::IClassFactory;
use windows::core::{GUID, IUnknown, Interface as _, Result, implement};

use crate::globals::SERVER_LOCK_COUNT;
use crate::text_service::JamoTextService;

#[implement(IClassFactory)]
pub struct JamoClassFactory;

impl IClassFactory_Impl for JamoClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        unsafe {
            if ppvobject.is_null() {
                return Err(windows::Win32::Foundation::E_POINTER.into());
            }
            *ppvobject = std::ptr::null_mut();

            // COM aggregation はサポートしない
            if punkouter.is_some() {
                return Err(windows::Win32::Foundation::CLASS_E_NOAGGREGATION.into());
            }

            let service = JamoTextService::new();
            let unknown: IUnknown = service.into();
            unknown.query(&*riid, ppvobject)
        }
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            SERVER_LOCK_COUNT.fetch_add(1, Ordering::SeqCst);
        } else {
            SERVER_LOCK_COUNT.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
