//! 変換セッション状態機械
//!
//! ローマ字入力〜変換〜候補選択〜確定までの状態を保持する。
//! プラットフォーム・キーバインドに依存しないモデル操作 API のみを提供し、
//! どのキーで何を起こすかはホスト (lite-dm2x の ime.lua、Windows の
//! composition.rs、C API 利用者) が決める。
//! 辞書・接続コストは保持せず、必要な操作の引数で受け取る (状態とロジックの分離)。

use crate::dict::Dictionary;
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

    /// 1文字入力 (ローマ字はひらがなに変換、その他はそのまま)。
    /// Converting 中は無効 (ホストが commit/revert してから入力する)。
    pub fn feed(&mut self, ch: char) -> Result<(), SessionError> {
        if self.state() == SessionState::Converting {
            return Err(SessionError::InvalidState);
        }
        if let crate::romaji::RomajiResult::Emit(s) = self.romaji.feed(ch) {
            self.kana.push_str(&s);
        }
        Ok(())
    }

    /// Backspace。未確定ローマ字→ひらがなの順で1文字消す。
    /// 消すものがなければ false (ホストは通常の Backspace として扱う)。
    pub fn backspace(&mut self) -> Result<bool, SessionError> {
        if self.state() == SessionState::Converting {
            return Err(SessionError::InvalidState);
        }
        match self.romaji.backspace() {
            BackspaceResult::DeletedPending => Ok(true),
            BackspaceResult::DeleteHiragana | BackspaceResult::Empty => {
                if self.kana.is_empty() {
                    Ok(false)
                } else {
                    self.kana.pop();
                    Ok(true)
                }
            }
        }
    }

    /// preedit 表示文字列。Input: かな+未確定ローマ字、Converting: 選択中候補の連結
    pub fn preedit(&self) -> String {
        if self.state() == SessionState::Input {
            format!("{}{}", self.kana, self.romaji.pending())
        } else {
            self.clauses.iter().map(|c| c.text()).collect()
        }
    }

    /// preedit 内のフォーカス範囲 (文字単位の (開始, 長さ))。
    /// Input 中はカーソル位置 (末尾, 0)。
    pub fn focused_range(&self) -> (usize, usize) {
        if self.state() == SessionState::Input {
            return (self.preedit().chars().count(), 0);
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

    /// かな漢字変換を実行して Converting に遷移する
    pub fn convert(
        &mut self,
        dict: &dyn Dictionary,
        conn: &dyn ConnectionCost,
        n: usize,
    ) -> Result<(), SessionError> {
        if self.state() == SessionState::Converting {
            return Err(SessionError::InvalidState);
        }
        self.kana.push_str(&self.romaji.flush());
        if self.kana.is_empty() {
            return Err(SessionError::EmptyInput);
        }
        let result = crate::convert_nbest(&self.kana, dict, conn, n);
        self.apply_result(result);
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
        self.apply_result(result);
        self.focus = focus.min(self.clauses.len().saturating_sub(1));
        Ok(())
    }

    fn apply_result(&mut self, result: ConvertResult) {
        self.boundaries = clause_boundaries(&result.clauses);
        self.clauses = result
            .clauses
            .into_iter()
            .map(|c| SessionClause {
                yomi: c.yomi,
                candidates: c.candidates,
                selected: 0,
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

    /// 変換を取り消して Input に戻る (かなは保持)
    pub fn revert(&mut self) -> Result<(), SessionError> {
        if self.state() != SessionState::Converting {
            return Err(SessionError::InvalidState);
        }
        self.clauses.clear();
        self.boundaries.clear();
        self.focus = 0;
        Ok(())
    }

    /// 確定。確定文字列を返してセッションを初期状態に戻す。
    /// Input 中はかな (+未確定ローマ字の flush 分) を確定する。
    pub fn commit(&mut self) -> String {
        let text = if self.state() == SessionState::Converting {
            self.preedit()
        } else {
            format!("{}{}", self.kana, self.romaji.flush())
        };
        self.reset();
        text
    }

    /// 全状態を破棄して初期状態に戻す
    pub fn reset(&mut self) {
        self.romaji.reset();
        self.kana.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::HashMapDict;
    use crate::viterbi::SimpleConnectionCost;

    fn feed_str(s: &mut Session, text: &str) {
        for ch in text.chars() {
            s.feed(ch).unwrap();
        }
    }

    fn converting_session(input: &str) -> (Session, HashMapDict) {
        let dict = HashMapDict::test_dict();
        let mut s = Session::new();
        feed_str(&mut s, input);
        s.convert(&dict, &SimpleConnectionCost, 5).unwrap();
        (s, dict)
    }

    #[test]
    fn test_new_session_is_empty_input() {
        let s = Session::new();
        assert_eq!(s.state(), SessionState::Input);
        assert_eq!(s.preedit(), "");
        assert!(s.clauses().is_empty());
    }

    #[test]
    fn test_feed_romaji_builds_kana() {
        let mut s = Session::new();
        s.feed('k').unwrap();
        assert_eq!(s.preedit(), "k"); // 未確定ローマ字も preedit に出る
        s.feed('y').unwrap();
        s.feed('o').unwrap();
        assert_eq!(s.preedit(), "きょ");
        s.feed('u').unwrap();
        assert_eq!(s.preedit(), "きょう");
    }

    #[test]
    fn test_feed_hiragana_passthrough() {
        let mut s = Session::new();
        s.feed('き').unwrap();
        s.feed('ょ').unwrap();
        assert_eq!(s.preedit(), "きょ");
    }

    #[test]
    fn test_backspace_pending_then_kana() {
        let mut s = Session::new();
        feed_str(&mut s, "kak");
        assert_eq!(s.preedit(), "かk");
        assert_eq!(s.backspace(), Ok(true)); // pending "k" を消す
        assert_eq!(s.preedit(), "か");
        assert_eq!(s.backspace(), Ok(true)); // かなを消す
        assert_eq!(s.preedit(), "");
        assert_eq!(s.backspace(), Ok(false)); // もう何もない
    }

    #[test]
    fn test_convert_moves_to_converting() {
        let (s, _dict) = converting_session("kyouha");
        assert_eq!(s.state(), SessionState::Converting);
        assert_eq!(s.clauses().len(), 1);
        assert_eq!(s.clauses()[0].yomi, "きょうは");
        assert_eq!(s.preedit(), "今日は");
        assert_eq!(s.focus(), 0);
    }

    #[test]
    fn test_convert_flushes_pending_n() {
        let dict = HashMapDict::test_dict();
        let mut s = Session::new();
        feed_str(&mut s, "kyoun"); // "n" が pending に残る
        s.convert(&dict, &SimpleConnectionCost, 5).unwrap();
        let all_yomi: String = s.clauses().iter().map(|c| c.yomi.as_str()).collect();
        assert_eq!(all_yomi, "きょうん"); // flush で「ん」が確定してから変換される
    }

    #[test]
    fn test_convert_empty_is_error() {
        let dict = HashMapDict::test_dict();
        let mut s = Session::new();
        assert_eq!(
            s.convert(&dict, &SimpleConnectionCost, 5),
            Err(SessionError::EmptyInput)
        );
    }

    #[test]
    fn test_feed_while_converting_is_error() {
        let (mut s, _dict) = converting_session("kyouha");
        assert_eq!(s.feed('a'), Err(SessionError::InvalidState));
    }

    #[test]
    fn test_select_candidate_changes_preedit() {
        let (mut s, _dict) = converting_session("kyouha");
        let candidates = s.clauses()[0].candidates.clone();
        let kyou_ha = candidates.iter().position(|c| c == "京は").unwrap();
        s.select(0, kyou_ha).unwrap();
        assert_eq!(s.preedit(), "京は");
        assert_eq!(s.clauses()[0].selected, kyou_ha);
        assert_eq!(s.select(0, 999), Err(SessionError::InvalidIndex));
    }

    #[test]
    fn test_focus_and_range() {
        // きょうはてんき → [今日は][天][木]
        let (mut s, _dict) = converting_session("kyouhatenki");
        assert_eq!(s.clauses().len(), 3);
        assert_eq!(s.focused_range(), (0, 3)); // 今日は
        s.set_focus(1).unwrap();
        assert_eq!(s.focused_range(), (3, 1)); // 天
        assert_eq!(s.set_focus(99), Err(SessionError::InvalidIndex));
    }

    #[test]
    fn test_resize_extends_focused_clause() {
        // [今日は][天][木] の文節0を1文字伸ばす → 読み「きょうはて」
        let (mut s, dict) = converting_session("kyouhatenki");
        s.resize(1, &dict, &SimpleConnectionCost, 5).unwrap();
        assert_eq!(s.clauses()[0].yomi, "きょうはて");
        assert_eq!(s.focus(), 0);
        // 全文節の読みを連結すると元のかなに戻る
        let all: String = s.clauses().iter().map(|c| c.yomi.as_str()).collect();
        assert_eq!(all, "きょうはてんき");
    }

    #[test]
    fn test_resize_rejects_last_clause_extend() {
        let (mut s, dict) = converting_session("kyouha");
        // 1文節のみ → 最後の文節は伸縮不可
        assert_eq!(
            s.resize(1, &dict, &SimpleConnectionCost, 5),
            Err(SessionError::InvalidResize)
        );
    }

    #[test]
    fn test_revert_returns_to_input_with_kana() {
        let (mut s, _dict) = converting_session("kyouha");
        s.revert().unwrap();
        assert_eq!(s.state(), SessionState::Input);
        assert_eq!(s.preedit(), "きょうは");
    }

    #[test]
    fn test_commit_converting_returns_selection_and_resets() {
        let (mut s, _dict) = converting_session("kyouha");
        assert_eq!(s.commit(), "今日は");
        assert_eq!(s.state(), SessionState::Input);
        assert_eq!(s.preedit(), "");
    }

    #[test]
    fn test_commit_input_returns_kana_with_flush() {
        let mut s = Session::new();
        feed_str(&mut s, "kyoun"); // pending "n"
        assert_eq!(s.commit(), "きょうん");
        assert_eq!(s.preedit(), "");
    }

    #[test]
    fn test_reset_clears_everything() {
        let (mut s, _dict) = converting_session("kyouha");
        s.reset();
        assert_eq!(s.state(), SessionState::Input);
        assert_eq!(s.preedit(), "");
        assert!(s.clauses().is_empty());
    }
}
