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

#[cfg(test)]
mod tests;

use crate::dict::Dictionary;
use crate::learning::LearningStore;
use crate::romaji::{BackspaceResult, RomajiConverter};
use crate::viterbi::{ConnectionCost, ConvertResult, clause_boundaries};

/// セッションの状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// かな入力中 (未変換)
    Input,
    /// 変換中 (文節・候補を保持)
    Converting,
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
        if let crate::romaji::RomajiResult::Emit(s) = self.romaji.feed(ch) {
            self.insert_at_cursor(&s);
        }
        Ok(())
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
        self.boundaries = clause_boundaries(&result.clauses);
        self.clauses = result
            .clauses
            .into_iter()
            .map(|c| {
                let mut candidates = c.candidates;
                if let Some(learned) = learning.and_then(|l| l.get(&c.yomi)) {
                    // 学習済み表層を候補の先頭へ (なければ先頭に挿入)
                    if let Some(pos) = candidates.iter().position(|s| s == learned) {
                        candidates.remove(pos);
                    }
                    candidates.insert(0, learned.to_string());
                }
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
        self.clauses.clear();
        self.boundaries.clear();
        self.focus = 0;
        self.cursor = self.kana.len();
        Ok(())
    }

    /// 確定。確定文字列を返してセッションを初期状態に戻す。
    /// Input 中はかな (+未確定ローマ字の flush 分) を確定する。
    /// `learning` を渡すと変換中の各文節の (読み→確定表層) が記録される。
    pub fn commit(&mut self, learning: Option<&mut LearningStore>) -> String {
        let text = if self.state() == SessionState::Converting {
            if let Some(store) = learning {
                for clause in &self.clauses {
                    store.record(&clause.yomi, clause.text());
                }
            }
            self.preedit()
        } else {
            self.flush_pending();
            self.kana.clone()
        };
        self.reset();
        text
    }

    /// 全状態を破棄して初期状態に戻す
    pub fn reset(&mut self) {
        self.romaji.reset();
        self.kana.clear();
        self.cursor = 0;
        self.clauses.clear();
        self.boundaries.clear();
        self.focus = 0;
    }
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
