//! 変換セッション状態機械
//!
//! ローマ字入力〜変換〜候補選択〜確定までの状態を保持する。
//! プラットフォーム・キーバインドに依存しないモデル操作 API のみを提供し、
//! どのキーで何を起こすかはホスト (lite-dm2x の ime.lua、Windows の
//! composition.rs、C API 利用者) が決める。
//! 辞書・接続コストは保持せず、必要な操作の引数で受け取る (状態とロジックの分離)。
//!
//! Input 中の preedit は「かな + カーソル位置の未確定ローマ字」で、カーソルは
//! かなの任意の文字間に置ける (途中挿入・削除)。未確定ローマ字はカーソル移動や
//! 変換の前に flush される (n → ん、未完成の子音は英字のまま)。
//!
//! 大文字 (Shift+英字) を入力すると確定まで英字モードになり、以後の文字は
//! ローマ字変換せずそのまま preedit に入る (mozc の一時英字モード準拠)。
//! `transform` は preedit 全体をカタカナ/ひらがな/半角英字/全角英字にした
//! 1 文節の Converting 状態を作る (F7/F10/Ctrl+I 等のキーに割り当てる想定)。

#[cfg(test)]
mod tests;

use crate::dict::Dictionary;
use crate::kana;
use crate::learning::LearningStore;
use crate::romaji::{BackspaceResult, RomajiConverter, kana_to_romaji};
use crate::viterbi::{ConnectionCost, ConvertResult, clause_boundaries};

/// セッションの状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// かな入力中 (未変換)
    Input,
    /// 変換中 (文節・候補を保持)
    Converting,
}

/// preedit 全体の文字種変換
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    /// ひらがな
    Hiragana,
    /// カタカナ
    Katakana,
    /// 半角英字 (打鍵ローマ字に戻す。繰り返すと小文字→大文字→先頭大文字を巡回)
    HalfAlpha,
    /// 全角英字 (同上)
    WideAlpha,
}

/// 変換中の1文節 (候補と選択状態)
#[derive(Debug, Clone)]
pub struct SessionClause {
    /// 読み
    pub yomi: String,
    /// 候補 (先頭が最良)
    pub candidates: Vec<String>,
    /// 選択中の候補インデックス
    pub selected: usize,
}

impl SessionClause {
    /// 現在選択中のテキスト
    pub fn text(&self) -> &str {
        self.candidates
            .get(self.selected)
            .map(|s| s.as_str())
            .unwrap_or(&self.yomi)
    }
}

/// 変換セッション
#[derive(Debug, Clone, Default)]
pub struct Session {
    romaji: RomajiConverter,
    /// 確定済みひらがな
    kana: String,
    /// kana 内のカーソル (バイト位置)。未確定ローマ字はここに挿入される
    cursor: usize,
    /// 変換中の文節 (空 = Input 状態)
    clauses: Vec<SessionClause>,
    /// kana への文節境界バイト位置
    boundaries: Vec<usize>,
    /// フォーカス中の文節
    focus: usize,
    /// 一時英字モード (大文字入力で on、確定/リセットで off)
    alpha: bool,
    /// Converting が transform 由来ならその種別 (繰り返しで候補を巡回する)
    transform: Option<Transform>,
    /// 部分確定で直前に確定した文節の表層 (残り文節の学習文脈)
    context_carry: String,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> SessionState {
        if self.clauses.is_empty() {
            SessionState::Input
        } else {
            SessionState::Converting
        }
    }

    fn ensure_input(&self) -> Result<(), SessionError> {
        if self.state() == SessionState::Converting {
            Err(SessionError::InvalidState)
        } else {
            Ok(())
        }
    }

    /// カーソル位置に文字列を挿入してカーソルを進める
    fn insert_at_cursor(&mut self, s: &str) {
        self.kana.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// 未確定ローマ字をカーソル位置に確定する
    fn flush_pending(&mut self) {
        let flushed = self.romaji.flush();
        self.insert_at_cursor(&flushed);
    }

    /// 1文字入力 (ローマ字はひらがなに変換、その他はそのまま)。
    /// Converting 中は無効 (ホストが commit/revert してから入力する)。
    pub fn feed(&mut self, ch: char) -> Result<(), SessionError> {
        self.ensure_input()?;
        if ch.is_ascii_uppercase() && !self.alpha {
            self.flush_pending();
            self.alpha = true;
        }
        if self.alpha {
            self.insert_at_cursor(&ch.to_string());
            return Ok(());
        }
        if let crate::romaji::RomajiResult::Emit(s) = self.romaji.feed(ch) {
            self.insert_at_cursor(&s);
        }
        Ok(())
    }

    /// 一時英字モードか
    pub fn is_alpha(&self) -> bool {
        self.alpha
    }

    /// 一時英字モードを明示的に切り替える (大文字入力によらない on/off)
    pub fn set_alpha(&mut self, on: bool) -> Result<(), SessionError> {
        self.ensure_input()?;
        self.flush_pending();
        self.alpha = on;
        Ok(())
    }

    /// preedit 全体を文字種変換した 1 文節の Converting 状態にする。
    /// 同じ種別を繰り返すと候補 (大小文字) を巡回する。
    /// Converting 中 (通常変換) は読み全体に対して適用する。
    pub fn transform(&mut self, kind: Transform) -> Result<(), SessionError> {
        if self.state() == SessionState::Converting {
            if self.transform == Some(kind) {
                let c = &mut self.clauses[0];
                c.selected = (c.selected + 1) % c.candidates.len();
                return Ok(());
            }
            self.clear_conversion();
        }
        self.flush_pending();
        if self.kana.is_empty() {
            return Err(SessionError::EmptyInput);
        }
        let candidates = match kind {
            Transform::Hiragana => vec![kana::to_hiragana(&self.kana)],
            Transform::Katakana => vec![kana::to_katakana(&self.kana)],
            Transform::HalfAlpha => alpha_variants(&kana_to_romaji(&self.kana), false),
            Transform::WideAlpha => alpha_variants(&kana_to_romaji(&self.kana), true),
        };
        self.set_single_clause(candidates);
        self.transform = Some(kind);
        Ok(())
    }

    /// 英字モードの変換: 打った文字列の幅・大小文字バリエーションを候補にする
    fn convert_alpha(&mut self) {
        let typed = self.kana.clone();
        let mut candidates = vec![typed.clone(), kana::to_wide(&typed)];
        for v in alpha_variants(&typed, false) {
            if !candidates.contains(&v) {
                candidates.push(v);
            }
        }
        self.set_single_clause(candidates);
        self.transform = Some(Transform::HalfAlpha);
    }

    fn set_single_clause(&mut self, candidates: Vec<String>) {
        self.boundaries = vec![self.kana.len()];
        self.clauses = vec![SessionClause {
            yomi: self.kana.clone(),
            candidates,
            selected: 0,
        }];
        self.focus = 0;
    }

    fn clear_conversion(&mut self) {
        self.clauses.clear();
        self.boundaries.clear();
        self.focus = 0;
        self.transform = None;
    }

    /// Backspace。未確定ローマ字→カーソル直前のかなの順で1文字消す。
    /// 消すものがなければ false (ホストは通常の Backspace として扱う)。
    pub fn backspace(&mut self) -> Result<bool, SessionError> {
        self.ensure_input()?;
        match self.romaji.backspace() {
            BackspaceResult::DeletedPending => Ok(true),
            BackspaceResult::DeleteHiragana | BackspaceResult::Empty => {
                let Some((start, _)) = self.kana[..self.cursor].char_indices().next_back() else {
                    return Ok(false);
                };
                self.kana.replace_range(start..self.cursor, "");
                self.cursor = start;
                Ok(true)
            }
        }
    }

    /// カーソル直後のかなを1文字消す (Delete)。未確定ローマ字は先に確定する。
    /// 消すものがなければ false。
    pub fn delete_forward(&mut self) -> Result<bool, SessionError> {
        self.ensure_input()?;
        self.flush_pending();
        let Some(ch) = self.kana[self.cursor..].chars().next() else {
            return Ok(false);
        };
        self.kana
            .replace_range(self.cursor..self.cursor + ch.len_utf8(), "");
        Ok(true)
    }

    /// カーソル位置 (preedit 内の文字インデックス)。Input 中のみ意味を持つ
    pub fn cursor(&self) -> usize {
        self.kana[..self.cursor].chars().count() + self.romaji.pending().chars().count()
    }

    /// カーソルを文字単位で移動する。未確定ローマ字は先に確定する。
    /// 端で動けなければ false。
    pub fn move_cursor(&mut self, delta: i32) -> Result<bool, SessionError> {
        self.ensure_input()?;
        self.flush_pending();
        let chars = self.kana[..self.cursor].chars().count() as i64;
        let total = self.kana.chars().count() as i64;
        let target = (chars + delta as i64).clamp(0, total);
        if target == chars {
            return Ok(false);
        }
        self.set_cursor_chars(target as usize);
        Ok(true)
    }

    /// カーソルを文字インデックスへ移動する (範囲外は端に丸める)
    pub fn set_cursor(&mut self, pos: usize) -> Result<(), SessionError> {
        self.ensure_input()?;
        self.flush_pending();
        self.set_cursor_chars(pos);
        Ok(())
    }

    fn set_cursor_chars(&mut self, pos: usize) {
        self.cursor = self
            .kana
            .char_indices()
            .nth(pos)
            .map(|(b, _)| b)
            .unwrap_or(self.kana.len());
    }

    /// preedit 表示文字列。Input: かな+未確定ローマ字、Converting: 選択中候補の連結
    pub fn preedit(&self) -> String {
        if self.state() == SessionState::Input {
            format!(
                "{}{}{}",
                &self.kana[..self.cursor],
                self.romaji.pending(),
                &self.kana[self.cursor..]
            )
        } else {
            self.clauses.iter().map(|c| c.text()).collect()
        }
    }

    /// preedit 内のフォーカス範囲 (文字単位の (開始, 長さ))。
    /// Input 中はカーソル位置 (長さ 0)。
    pub fn focused_range(&self) -> (usize, usize) {
        if self.state() == SessionState::Input {
            return (self.cursor(), 0);
        }
        let mut from = 0;
        for (i, clause) in self.clauses.iter().enumerate() {
            let len = clause.text().chars().count();
            if i == self.focus {
                return (from, len);
            }
            from += len;
        }
        (from, 0)
    }

    /// かな漢字変換を実行して Converting に遷移する。
    /// `learning` を渡すと学習済み表層が各文節候補の先頭に来る。
    pub fn convert(
        &mut self,
        dict: &dyn Dictionary,
        conn: &dyn ConnectionCost,
        n: usize,
        learning: Option<&LearningStore>,
    ) -> Result<(), SessionError> {
        self.ensure_input()?;
        self.flush_pending();
        if self.kana.is_empty() {
            return Err(SessionError::EmptyInput);
        }
        if self.alpha {
            self.convert_alpha();
            return Ok(());
        }
        let result = crate::convert_nbest(&self.kana, dict, conn, n);
        self.apply_result(result, learning);
        self.focus = 0;
        Ok(())
    }

    /// フォーカス文節を delta 文字分伸縮して再変換する
    pub fn resize(
        &mut self,
        delta: i32,
        dict: &dyn Dictionary,
        conn: &dyn ConnectionCost,
        n: usize,
        learning: Option<&LearningStore>,
    ) -> Result<(), SessionError> {
        if self.state() != SessionState::Converting {
            return Err(SessionError::InvalidState);
        }
        let new_boundaries =
            crate::resize_boundaries(&self.kana, &self.boundaries, self.focus, delta)
                .ok_or(SessionError::InvalidResize)?;
        let result = crate::try_convert_with_boundaries(&self.kana, &new_boundaries, dict, conn, n)
            .map_err(|_| SessionError::InvalidResize)?;
        let focus = self.focus;
        self.apply_result(result, learning);
        self.focus = focus.min(self.clauses.len().saturating_sub(1));
        Ok(())
    }

    fn apply_result(&mut self, result: ConvertResult, learning: Option<&LearningStore>) {
        self.transform = None;
        self.boundaries = clause_boundaries(&result.clauses);
        // 左から順に、直前文節の (学習適用後の) 表層を文脈として学習を引く
        let mut context = self.context_carry.clone();
        self.clauses = result
            .clauses
            .into_iter()
            .map(|c| {
                let mut candidates = c.candidates;
                if let Some(learned) = learning.and_then(|l| l.get(&context, &c.yomi)) {
                    // 学習済み表層を候補の先頭へ (なければ先頭に挿入)
                    if let Some(pos) = candidates.iter().position(|s| s == learned) {
                        candidates.remove(pos);
                    }
                    candidates.insert(0, learned.to_string());
                }
                context = candidates.first().cloned().unwrap_or_default();
                SessionClause {
                    yomi: c.yomi,
                    candidates,
                    selected: 0,
                }
            })
            .collect();
    }

    /// 変換中の文節一覧
    pub fn clauses(&self) -> &[SessionClause] {
        &self.clauses
    }

    /// 文節 i の候補 j を選択する
    pub fn select(&mut self, clause: usize, candidate: usize) -> Result<(), SessionError> {
        let c = self
            .clauses
            .get_mut(clause)
            .ok_or(SessionError::InvalidIndex)?;
        if candidate >= c.candidates.len() {
            return Err(SessionError::InvalidIndex);
        }
        c.selected = candidate;
        Ok(())
    }

    pub fn focus(&self) -> usize {
        self.focus
    }

    pub fn set_focus(&mut self, focus: usize) -> Result<(), SessionError> {
        if self.state() != SessionState::Converting || focus >= self.clauses.len() {
            return Err(SessionError::InvalidIndex);
        }
        self.focus = focus;
        Ok(())
    }

    /// 変換を取り消して Input に戻る (かなは保持、カーソルは末尾)
    pub fn revert(&mut self) -> Result<(), SessionError> {
        if self.state() != SessionState::Converting {
            return Err(SessionError::InvalidState);
        }
        self.clear_conversion();
        self.cursor = self.kana.len();
        Ok(())
    }

    /// 確定。確定文字列を返してセッションを初期状態に戻す。
    /// Input 中はかな (+未確定ローマ字の flush 分) を確定する。
    /// `learning` を渡すと変換中の各文節の (読み→確定表層) が記録される。
    pub fn commit(&mut self, learning: Option<&mut LearningStore>) -> String {
        let text = if self.state() == SessionState::Converting {
            // 英字変換 (打鍵に戻しただけ) は学習しない
            let learnable = !matches!(
                self.transform,
                Some(Transform::HalfAlpha | Transform::WideAlpha)
            );
            if let Some(store) = learning
                && learnable
            {
                Self::record_clauses(store, &self.context_carry, &self.clauses);
            }
            self.preedit()
        } else {
            self.flush_pending();
            self.kana.clone()
        };
        self.reset();
        text
    }

    /// フォーカス文節までを確定して返し、残りの文節は変換中のまま残す。
    /// 残りがなければ (Input 中も) `commit` と同じ。
    pub fn commit_prefix(&mut self, learning: Option<&mut LearningStore>) -> String {
        let n = self.focus + 1;
        if self.state() != SessionState::Converting || n >= self.clauses.len() {
            return self.commit(learning);
        }
        let head: Vec<SessionClause> = self.clauses.drain(..n).collect();
        let text: String = head.iter().map(|c| c.text()).collect();
        if let Some(store) = learning {
            Self::record_clauses(store, &self.context_carry, &head);
        }
        self.context_carry = head
            .last()
            .map(|c| c.text().to_string())
            .unwrap_or_default();
        let offset = self.boundaries[n - 1];
        self.boundaries.drain(..n);
        for b in &mut self.boundaries {
            *b -= offset;
        }
        self.kana.replace_range(..offset, "");
        self.cursor = self.kana.len();
        self.focus = 0;
        text
    }

    fn record_clauses(store: &mut LearningStore, context: &str, clauses: &[SessionClause]) {
        let mut context = context;
        for clause in clauses {
            store.record(context, &clause.yomi, clause.text());
            context = clause.text();
        }
    }

    /// 全状態を破棄して初期状態に戻す
    pub fn reset(&mut self) {
        self.romaji.reset();
        self.kana.clear();
        self.cursor = 0;
        self.clear_conversion();
        self.alpha = false;
        self.context_carry.clear();
    }
}

/// 英字列の大小文字バリエーション (打った通り → 小文字 → 大文字 → 先頭大文字、重複除去)
fn alpha_variants(typed: &str, wide: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in [
        typed.to_string(),
        typed.to_ascii_lowercase(),
        typed.to_ascii_uppercase(),
        kana::capitalize(typed),
    ] {
        let v = if wide { kana::to_wide(&v) } else { v };
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// セッション操作のエラー
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// この状態では実行できない操作
    InvalidState,
    /// 入力が空
    EmptyInput,
    /// 文節/候補インデックスが範囲外
    InvalidIndex,
    /// 伸縮できない (端の文節など)
    InvalidResize,
}
