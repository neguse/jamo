/// 候補ウィンドウ (OS 標準 UI)
///
/// MVP: ITfCandidateListUIElementBehavior で OS 標準の候補ウィンドウを使用。
/// 将来的にはカスタム候補ウィンドウに置き換え可能。
use windows::Win32::Foundation::BOOL;
use windows::Win32::UI::TextServices::*;
use windows::core::{BSTR, GUID, Result, implement};

#[implement(
    ITfCandidateListUIElementBehavior,
    ITfCandidateListUIElement,
    ITfUIElement
)]
pub struct JamoCandidateListUI {
    candidates: Vec<String>,
    selection: std::cell::Cell<u32>,
    shown: std::cell::Cell<bool>,
}

impl JamoCandidateListUI {
    pub fn new(candidates: Vec<String>, selection: u32) -> Self {
        Self {
            candidates,
            selection: std::cell::Cell::new(selection),
            shown: std::cell::Cell::new(false),
        }
    }

    pub fn update(&mut self, candidates: Vec<String>, selection: u32) {
        self.candidates = candidates;
        self.selection.set(selection);
    }
}

// --- ITfUIElement ---

impl ITfUIElement_Impl for JamoCandidateListUI_Impl {
    fn GetDescription(&self, pbstrdescription: *mut BSTR) -> Result<()> {
        unsafe {
            *pbstrdescription = BSTR::from("Jamo 候補");
        }
        Ok(())
    }

    fn GetGUID(&self, pguid: *mut GUID) -> Result<()> {
        unsafe {
            *pguid = GUID::zeroed();
        }
        Ok(())
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        self.shown.set(bshow.as_bool());
        Ok(())
    }

    fn IsShown(&self, pbshow: *mut BOOL) -> Result<()> {
        unsafe {
            *pbshow = BOOL(self.shown.get() as i32);
        }
        Ok(())
    }
}

// --- ITfCandidateListUIElement ---

impl ITfCandidateListUIElement_Impl for JamoCandidateListUI_Impl {
    fn GetUpdatedFlags(&self, pdwflags: *mut u32) -> Result<()> {
        unsafe {
            *pdwflags = 0x01 | 0x04; // TF_CLUIE_STRING | TF_CLUIE_SELECTION
        }
        Ok(())
    }

    fn GetDocumentMgr(&self, _ppdim: *mut Option<ITfDocumentMgr>) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn GetCount(&self, pucount: *mut u32) -> Result<()> {
        unsafe {
            *pucount = self.candidates.len() as u32;
        }
        Ok(())
    }

    fn GetSelection(&self, puindex: *mut u32) -> Result<()> {
        unsafe {
            *puindex = self.selection.get();
        }
        Ok(())
    }

    fn GetString(&self, uindex: u32, pstr: *mut BSTR) -> Result<()> {
        let idx = uindex as usize;
        if idx >= self.candidates.len() {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        unsafe {
            *pstr = BSTR::from(&self.candidates[idx]);
        }
        Ok(())
    }

    fn GetPageIndex(&self, _pindex: *mut u32, _usize: u32, _pupagecnt: *mut u32) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn GetCurrentPage(&self, _pupage: *mut u32) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }
}

// --- ITfCandidateListUIElementBehavior ---

impl ITfCandidateListUIElementBehavior_Impl for JamoCandidateListUI_Impl {
    fn SetSelection(&self, nindex: u32) -> Result<()> {
        if (nindex as usize) < self.candidates.len() {
            self.selection.set(nindex);
            Ok(())
        } else {
            Err(windows::Win32::Foundation::E_INVALIDARG.into())
        }
    }

    fn Finalize(&self) -> Result<()> {
        Ok(())
    }

    fn Abort(&self) -> Result<()> {
        Ok(())
    }
}
