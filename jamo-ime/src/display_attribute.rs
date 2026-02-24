/// DisplayAttribute 実装
///
/// Composition テキストの見た目 (下線) を制御する 3 種類の表示属性。
use windows::Win32::UI::TextServices::*;
use windows::core::{BSTR, GUID, Result, implement};

use crate::globals::{GUID_DA_CONVERTED, GUID_DA_FOCUSED, GUID_DA_INPUT};

/// 表示属性の種類
#[derive(Debug, Clone, Copy)]
enum AttributeKind {
    Input,
    Converted,
    Focused,
}

// --- ITfDisplayAttributeProvider ---

#[implement(ITfDisplayAttributeProvider)]
pub struct JamoDisplayAttributeProvider;

impl JamoDisplayAttributeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl ITfDisplayAttributeProvider_Impl for JamoDisplayAttributeProvider_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let enumerator = DisplayAttributeInfoEnum {
            index: std::cell::Cell::new(0),
        };
        Ok(enumerator.into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        unsafe {
            let guid = &*guid;
            if *guid == GUID_DA_INPUT {
                Ok(JamoDisplayAttributeInfo(AttributeKind::Input).into())
            } else if *guid == GUID_DA_CONVERTED {
                Ok(JamoDisplayAttributeInfo(AttributeKind::Converted).into())
            } else if *guid == GUID_DA_FOCUSED {
                Ok(JamoDisplayAttributeInfo(AttributeKind::Focused).into())
            } else {
                Err(windows::Win32::Foundation::E_INVALIDARG.into())
            }
        }
    }
}

// --- IEnumTfDisplayAttributeInfo ---

#[implement(IEnumTfDisplayAttributeInfo)]
struct DisplayAttributeInfoEnum {
    index: std::cell::Cell<usize>,
}

impl IEnumTfDisplayAttributeInfo_Impl for DisplayAttributeInfoEnum_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let cloned = DisplayAttributeInfoEnum {
            index: self.index.clone(),
        };
        Ok(cloned.into())
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        let kinds = [
            AttributeKind::Input,
            AttributeKind::Converted,
            AttributeKind::Focused,
        ];
        let mut fetched = 0u32;
        let mut idx = self.index.get();

        for i in 0..ulcount as usize {
            if idx >= kinds.len() {
                break;
            }
            let info: ITfDisplayAttributeInfo = JamoDisplayAttributeInfo(kinds[idx]).into();
            unsafe {
                *rginfo.add(i) = Some(info);
            }
            idx += 1;
            fetched += 1;
        }

        self.index.set(idx);
        unsafe {
            if !pcfetched.is_null() {
                *pcfetched = fetched;
            }
        }

        if fetched == ulcount {
            Ok(())
        } else {
            Err(windows::Win32::Foundation::S_FALSE.into())
        }
    }

    fn Reset(&self) -> Result<()> {
        self.index.set(0);
        Ok(())
    }

    fn Skip(&self, ulcount: u32) -> Result<()> {
        let new_idx = self.index.get() + ulcount as usize;
        self.index.set(new_idx.min(3));
        Ok(())
    }
}

// --- ITfDisplayAttributeInfo ---

#[implement(ITfDisplayAttributeInfo)]
struct JamoDisplayAttributeInfo(AttributeKind);

impl ITfDisplayAttributeInfo_Impl for JamoDisplayAttributeInfo_Impl {
    fn GetGUID(&self, pguid: *mut GUID) -> Result<()> {
        let guid = match self.0 {
            AttributeKind::Input => GUID_DA_INPUT,
            AttributeKind::Converted => GUID_DA_CONVERTED,
            AttributeKind::Focused => GUID_DA_FOCUSED,
        };
        unsafe {
            *pguid = guid;
        }
        Ok(())
    }

    fn GetDescription(&self, pbstrDesc: *mut BSTR) -> Result<()> {
        let desc = match self.0 {
            AttributeKind::Input => "Jamo Input",
            AttributeKind::Converted => "Jamo Converted",
            AttributeKind::Focused => "Jamo Focused",
        };
        unsafe {
            *pbstrDesc = BSTR::from(desc);
        }
        Ok(())
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        let attr = match self.0 {
            AttributeKind::Input => TF_DISPLAYATTRIBUTE {
                crText: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0), // TF_CT_NONE
                    ..Default::default()
                },
                crBk: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                lsStyle: TF_LS_DOT,
                fBoldLine: false.into(),
                crLine: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                bAttr: TF_DA_ATTR_INPUT,
            },
            AttributeKind::Converted => TF_DISPLAYATTRIBUTE {
                crText: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                crBk: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                lsStyle: TF_LS_SOLID,
                fBoldLine: false.into(),
                crLine: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                bAttr: TF_DA_ATTR_CONVERTED,
            },
            AttributeKind::Focused => TF_DISPLAYATTRIBUTE {
                crText: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                crBk: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                lsStyle: TF_LS_SOLID,
                fBoldLine: true.into(),
                crLine: TF_DA_COLOR {
                    r#type: TF_DA_COLORTYPE(0),
                    ..Default::default()
                },
                bAttr: TF_DA_ATTR_TARGET_CONVERTED,
            },
        };
        unsafe {
            *pda = attr;
        }
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}
