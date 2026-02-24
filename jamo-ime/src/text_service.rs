/// TSF テキストサービス (ITfTextInputProcessorEx + ITfKeyEventSink)
///
/// IME のメインエントリポイント。有効化/無効化、キーイベント処理を管理。
use std::cell::RefCell;
use std::sync::OnceLock;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::TextServices::*;
use windows::core::{BOOL, GUID, Interface as _, Ref, Result, implement};

use crate::composition::{Action, InputState};
use crate::edit_session;
use crate::engine::ImeEngine;
use crate::globals::{GUID_PRESERVED_KEY_ALT_BACKQUOTE, GUID_PRESERVED_KEY_ONOFF};

/// PreservedKey: 半角/全角 (VK_KANJI = 0x19)
const TF_PRESERVED_KEY_ONOFF: TF_PRESERVEDKEY = TF_PRESERVEDKEY {
    uVKey: 0x19, // VK_KANJI
    uModifiers: 0,
};

/// PreservedKey: Alt+` (HHKB 等の半角/全角キーがないキーボード用)
const TF_PRESERVED_KEY_ALT_BACKQUOTE: TF_PRESERVEDKEY = TF_PRESERVEDKEY {
    uVKey: 0xC0,        // VK_OEM_3 (backquote/tilde)
    uModifiers: 0x0001, // TF_MOD_ALT
};

#[implement(
    ITfTextInputProcessorEx,
    ITfTextInputProcessor,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider
)]
pub struct JamoTextService {
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    client_id: RefCell<u32>,
    composition: RefCell<Option<ITfComposition>>,
    state: RefCell<InputState>,
    engine: OnceLock<ImeEngine>,
}

impl JamoTextService {
    pub fn new() -> Self {
        // IME 選択 = 日本語入力したい、なので最初から Composing で起動
        Self {
            thread_mgr: RefCell::new(None),
            client_id: RefCell::new(0),
            composition: RefCell::new(None),
            state: RefCell::new(InputState::composing()),
            engine: OnceLock::new(),
        }
    }

    fn vk_to_char(wparam: WPARAM) -> Option<char> {
        let vk = wparam.0 as u16;
        match vk {
            0x41..=0x5A => Some((b'a' + (vk - 0x41) as u8) as char),
            // OEM キー
            0xBE => Some('.'),
            0xBC => Some(','),
            0xBF => Some('/'),
            0xDB => Some('['),
            0xDD => Some(']'),
            0xBD => Some('-'),
            0xBA => Some(';'),
            0xBB => Some('='),
            _ => None,
        }
    }

    /// self から ITfCompositionSink を取得 (vtable 経由)
    fn get_composition_sink(&self) -> Result<ITfCompositionSink> {
        unsafe {
            let self_ptr = self as *const _ as *mut std::ffi::c_void;
            let mut sink_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            crate::globals::com_query_interface(self_ptr, &ITfCompositionSink::IID, &mut sink_ptr)
                .ok()?;
            Ok(ITfCompositionSink::from_raw(sink_ptr))
        }
    }

    fn process_key(&self, context: &ITfContext, wparam: WPARAM, shift: bool) -> Result<bool> {
        let vk = wparam.0 as u16;
        let ch = Self::vk_to_char(wparam);

        let action = self.state.borrow_mut().handle_key(vk, ch, shift);
        let client_id = *self.client_id.borrow();

        match action {
            Action::UpdateComposition(text) => {
                let sink = self.get_composition_sink()?;
                edit_session::request_update_composition(
                    context,
                    client_id,
                    &self.composition,
                    &text,
                    &sink,
                )?;
                Ok(true)
            }
            Action::EndComposition(text) => {
                edit_session::request_end_composition(
                    context,
                    client_id,
                    &self.composition,
                    &text,
                )?;
                self.state.borrow_mut().reset_to_composing();
                Ok(true)
            }
            Action::StartConversion(hiragana) => {
                if let Some(engine) = self.engine.get() {
                    let result = engine.convert(&hiragana, 9);
                    self.state
                        .borrow_mut()
                        .set_conversion_result(hiragana, result);
                    let text = self.state.borrow().display_text();
                    let sink = self.get_composition_sink()?;
                    edit_session::request_update_composition(
                        context,
                        client_id,
                        &self.composition,
                        &text,
                        &sink,
                    )?;
                }
                Ok(true)
            }
            Action::UpdateConversion => {
                let text = self.state.borrow().display_text();
                let sink = self.get_composition_sink()?;
                edit_session::request_update_composition(
                    context,
                    client_id,
                    &self.composition,
                    &text,
                    &sink,
                )?;
                Ok(true)
            }
            Action::CancelConversion(hiragana) => {
                let sink = self.get_composition_sink()?;
                edit_session::request_update_composition(
                    context,
                    client_id,
                    &self.composition,
                    &hiragana,
                    &sink,
                )?;
                self.state.borrow_mut().revert_to_composing(hiragana);
                Ok(true)
            }
            Action::ToggleMode => {
                self.state.borrow_mut().toggle_mode();
                Ok(true)
            }
            Action::Noop => Ok(false),
        }
    }
}

// --- ITfTextInputProcessorEx ---

impl ITfTextInputProcessorEx_Impl for JamoTextService_Impl {
    fn ActivateEx(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        // Ref::clone() returns Option<ITfThreadMgr>
        *self.thread_mgr.borrow_mut() = ptim.clone();
        *self.client_id.borrow_mut() = tid;

        // IPC サーバーに接続 (辞書は jamo serve --ime 側で保持)
        let _ = self.engine.set(ImeEngine::connect());

        // キーイベント登録
        unsafe {
            let thread_mgr = ptim.clone().ok_or(windows::Win32::Foundation::E_FAIL)?;
            let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;

            // self (_Impl) から ITfKeyEventSink を取得
            // _Impl の先頭は IUnknown vtable なので vtable 経由で QueryInterface
            let self_ptr = self as *const _ as *mut std::ffi::c_void;
            let mut sink_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            crate::globals::com_query_interface(self_ptr, &ITfKeyEventSink::IID, &mut sink_ptr)
                .ok()?;
            let sink = ITfKeyEventSink::from_raw(sink_ptr);

            keystroke_mgr.AdviseKeyEventSink(tid, &sink, true)?;

            // 半角/全角キー登録
            keystroke_mgr.PreserveKey(
                tid,
                &GUID_PRESERVED_KEY_ONOFF,
                &TF_PRESERVED_KEY_ONOFF,
                &[],
            )?;

            // Alt+` キー登録 (HHKB 等)
            keystroke_mgr.PreserveKey(
                tid,
                &GUID_PRESERVED_KEY_ALT_BACKQUOTE,
                &TF_PRESERVED_KEY_ALT_BACKQUOTE,
                &[],
            )?;
        }

        Ok(())
    }
}

// --- ITfTextInputProcessor ---

impl ITfTextInputProcessor_Impl for JamoTextService_Impl {
    fn Activate(&self, ptim: Ref<'_, ITfThreadMgr>, tid: u32) -> Result<()> {
        self.ActivateEx(ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        let client_id = *self.client_id.borrow();
        if let Some(thread_mgr) = self.thread_mgr.borrow().as_ref() {
            unsafe {
                let keystroke_mgr: ITfKeystrokeMgr = thread_mgr.cast()?;
                keystroke_mgr.UnadviseKeyEventSink(client_id).ok();
                keystroke_mgr
                    .UnpreserveKey(&GUID_PRESERVED_KEY_ONOFF, &TF_PRESERVED_KEY_ONOFF)
                    .ok();
                keystroke_mgr
                    .UnpreserveKey(
                        &GUID_PRESERVED_KEY_ALT_BACKQUOTE,
                        &TF_PRESERVED_KEY_ALT_BACKQUOTE,
                    )
                    .ok();
            }
        }
        *self.thread_mgr.borrow_mut() = None;
        *self.client_id.borrow_mut() = 0;
        *self.state.borrow_mut() = InputState::composing();
        Ok(())
    }
}

// --- ITfKeyEventSink ---

impl ITfKeyEventSink_Impl for JamoTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Ref<'_, ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let shift = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x10) < 0 };
        let eaten = self.state.borrow().wants_key(wparam.0 as u16, shift);
        Ok(BOOL(eaten as i32))
    }

    fn OnTestKeyUp(
        &self,
        _pic: Ref<'_, ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnKeyDown(&self, pic: Ref<'_, ITfContext>, wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        let shift = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(0x10) < 0 };
        let context = pic.clone().ok_or(windows::Win32::Foundation::E_FAIL)?;
        let eaten = self.process_key(&context, wparam, shift)?;
        Ok(BOOL(eaten as i32))
    }

    fn OnKeyUp(&self, _pic: Ref<'_, ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnPreservedKey(&self, _pic: Ref<'_, ITfContext>, rguid: *const GUID) -> Result<BOOL> {
        unsafe {
            if *rguid == GUID_PRESERVED_KEY_ONOFF || *rguid == GUID_PRESERVED_KEY_ALT_BACKQUOTE {
                self.state.borrow_mut().toggle_mode();
                Ok(BOOL(1))
            } else {
                Ok(BOOL(0))
            }
        }
    }
}

// --- ITfCompositionSink ---

impl ITfCompositionSink_Impl for JamoTextService_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Ref<'_, ITfComposition>,
    ) -> Result<()> {
        *self.composition.borrow_mut() = None;
        *self.state.borrow_mut() = InputState::composing();
        Ok(())
    }
}

// --- ITfDisplayAttributeProvider (delegate) ---

impl ITfDisplayAttributeProvider_Impl for JamoTextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        crate::display_attribute::enum_display_attribute_info()
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        crate::display_attribute::get_display_attribute_info(guid)
    }
}
