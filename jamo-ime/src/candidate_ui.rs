/// 候補ウィンドウ (OS 標準 UI)
///
/// MVP: ITfCandidateListUIElementBehavior で OS 標準の候補ウィンドウを使用。
use windows::Win32::UI::TextServices::*;
use windows::core::{BOOL, BSTR, GUID, Result, implement};

use crate::globals::ComObjectGuard;

#[implement(
    ITfCandidateListUIElementBehavior,
    ITfCandidateListUIElement,
    ITfUIElement
)]
pub struct JamoCandidateListUI {
    candidates: Vec<String>,
    selection: std::cell::Cell<u32>,
    shown: std::cell::Cell<bool>,
    _lifetime: ComObjectGuard,
}

impl JamoCandidateListUI {
    pub fn new(candidates: Vec<String>, selection: u32) -> Self {
        Self {
            candidates,
            selection: std::cell::Cell::new(selection),
            shown: std::cell::Cell::new(false),
            _lifetime: ComObjectGuard::new(),
        }
    }

    pub fn update(&mut self, candidates: Vec<String>, selection: u32) {
        self.candidates = candidates;
        self.selection.set(selection);
    }
}

// --- ITfUIElement ---

impl ITfUIElement_Impl for JamoCandidateListUI_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("Jamo 候補"))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID::zeroed())
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        self.shown.set(bshow.as_bool());
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        Ok(BOOL(self.shown.get() as i32))
    }
}

// --- ITfCandidateListUIElement ---

impl ITfCandidateListUIElement_Impl for JamoCandidateListUI_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(TF_CLUIE_STRING | TF_CLUIE_SELECTION)
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn GetCount(&self) -> Result<u32> {
        Ok(self.candidates.len() as u32)
    }

    fn GetSelection(&self) -> Result<u32> {
        Ok(self.selection.get())
    }

    fn GetString(&self, uindex: u32) -> Result<BSTR> {
        let idx = uindex as usize;
        if idx >= self.candidates.len() {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        Ok(BSTR::from(&self.candidates[idx]))
    }

    fn GetPageIndex(&self, _pindex: *mut u32, _usize: u32, _pupagecnt: *mut u32) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
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
