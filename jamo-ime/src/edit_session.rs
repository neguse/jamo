/// EditSession パターン
///
/// TSF ではテキスト変更は全て EditSession コールバック経由で行う。
use std::cell::RefCell;

use windows::Win32::UI::TextServices::*;
use windows::core::{Interface as _, Result, implement};

/// Composition テキスト更新の EditSession を要求
pub fn request_update_composition(
    context: &ITfContext,
    client_id: u32,
    composition: &RefCell<Option<ITfComposition>>,
    text: &str,
) -> Result<()> {
    let session = UpdateCompositionSession {
        context: context.clone(),
        composition: composition.clone(),
        text: text.to_string(),
        client_id,
    };
    let session: ITfEditSession = session.into();
    unsafe {
        context.RequestEditSession(client_id, &session, TF_ES_READWRITE | TF_ES_SYNC)?;
    }
    Ok(())
}

/// Composition 終了の EditSession を要求
pub fn request_end_composition(
    context: &ITfContext,
    client_id: u32,
    composition: &RefCell<Option<ITfComposition>>,
    text: &str,
) -> Result<()> {
    let session = EndCompositionSession {
        context: context.clone(),
        composition: composition.clone(),
        text: text.to_string(),
        client_id,
    };
    let session: ITfEditSession = session.into();
    unsafe {
        context.RequestEditSession(client_id, &session, TF_ES_READWRITE | TF_ES_SYNC)?;
    }
    Ok(())
}

// --- UpdateCompositionSession ---

#[implement(ITfEditSession)]
struct UpdateCompositionSession {
    context: ITfContext,
    composition: RefCell<Option<ITfComposition>>,
    text: String,
    client_id: u32,
}

impl ITfEditSession_Impl for UpdateCompositionSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let mut comp = self.composition.borrow_mut();

            if comp.is_none() {
                // Composition がまだない → 開始
                let context_composition: ITfContextComposition = self.context.cast()?;
                let sink: ITfCompositionSink = self
                    .context
                    .cast()
                    .ok()
                    .and_then(|_: ITfContext| None)
                    .unwrap_or_else(|| {
                        // CompositionSink は text_service が持つが、ここでは簡易版
                        // 実際には JamoTextService の ITfCompositionSink を使う
                        todo!("Need composition sink from text_service")
                    });

                let range = insert_text_at_selection(&self.context, ec, &self.text)?;
                let new_comp = context_composition.StartComposition(ec, &range, &sink)?;
                *comp = Some(new_comp);
            } else if let Some(composition) = comp.as_ref() {
                // 既存の Composition テキストを更新
                let range = composition.GetRange()?;
                let text_u16: Vec<u16> = self.text.encode_utf16().collect();
                range.SetText(ec, 0, &text_u16)?;
            }
        }
        Ok(())
    }
}

// --- EndCompositionSession ---

#[implement(ITfEditSession)]
struct EndCompositionSession {
    context: ITfContext,
    composition: RefCell<Option<ITfComposition>>,
    text: String,
    client_id: u32,
}

impl ITfEditSession_Impl for EndCompositionSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        unsafe {
            let mut comp = self.composition.borrow_mut();
            if let Some(composition) = comp.take() {
                if !self.text.is_empty() {
                    let range = composition.GetRange()?;
                    let text_u16: Vec<u16> = self.text.encode_utf16().collect();
                    range.SetText(ec, 0, &text_u16)?;
                }
                composition.EndComposition(ec)?;
            }
        }
        Ok(())
    }
}

// --- ヘルパー ---

unsafe fn insert_text_at_selection(context: &ITfContext, ec: u32, text: &str) -> Result<ITfRange> {
    let insert_at_selection: ITfInsertAtSelection = context.cast()?;
    let text_u16: Vec<u16> = text.encode_utf16().collect();
    let range = insert_at_selection.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &text_u16)?;
    range.SetText(ec, 0, &text_u16)?;
    Ok(range)
}
