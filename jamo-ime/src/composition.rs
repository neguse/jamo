/// Composition 状態マシン (Platform 非依存)
///
/// IME の入力状態 (Direct / Composing / Converting) を管理する。
/// TSF に依存しないため Linux でもテスト可能。
use jamo_core::romaji::{BackspaceResult, RomajiConverter, RomajiResult};
use jamo_core::viterbi::ConvertResult;

/// キー入力に対するアクション (TSF 側への操作指示)
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Composition テキストを更新
    UpdateComposition(String),
    /// テキストを確定してアプリに送信
    EndComposition(String),
    /// 変換開始要求 (engine.convert を呼ぶ)
    StartConversion(String),
    /// 変換結果の表示更新
    UpdateConversion,
    /// 変換を取り消して Composing に戻る
    CancelConversion(String),
    /// Direct ↔ Composing モード切替
    ToggleMode,
    /// 何もしない
    Noop,
}

/// IME の入力状態
#[derive(Debug)]
pub enum InputState {
    /// パススルー (IME オフ)
    Direct,
    /// ひらがな入力中
    Composing {
        hiragana: String,
        romaji: RomajiConverter,
    },
    /// 変換中
    Converting {
        input: String,
        result: ConvertResult,
        focused_clause: usize,
        candidate_indices: Vec<usize>,
        boundaries: Vec<usize>,
    },
}

/// 仮想キーコード (Windows VK_* 互換)
pub mod vk {
    pub const BACK: u16 = 0x08;
    pub const RETURN: u16 = 0x0D;
    pub const ESCAPE: u16 = 0x1B;
    pub const SPACE: u16 = 0x20;
    pub const LEFT: u16 = 0x25;
    pub const UP: u16 = 0x26;
    pub const RIGHT: u16 = 0x27;
    pub const DOWN: u16 = 0x28;
}

impl InputState {
    /// 初期状態
    pub fn new() -> Self {
        InputState::Direct
    }

    /// IME がこのキーを処理すべきか (OnTestKeyDown 用, 副作用なし)
    pub fn wants_key(&self, vk_code: u16, shift: bool) -> bool {
        match self {
            InputState::Direct => false,
            InputState::Composing { hiragana, romaji } => {
                // 入力中テキストがある場合
                let has_content = !hiragana.is_empty() || !romaji.pending().is_empty();
                match vk_code {
                    // 英数字キーは常に受け取る
                    0x41..=0x5A => true, // A-Z
                    // 句読点
                    0xBE | 0xBC | 0xBF | 0xDB | 0xDD | 0xBD => true, // . , / [ ] -
                    0xBA | 0xBB => true,                             // ; =
                    // 内容がある場合のみ
                    vk::BACK | vk::RETURN | vk::ESCAPE => has_content,
                    vk::SPACE => has_content,
                    _ => false,
                }
            }
            InputState::Converting { .. } => {
                matches!(
                    vk_code,
                    vk::SPACE | vk::RETURN | vk::ESCAPE | vk::LEFT | vk::RIGHT | vk::UP | vk::DOWN
                ) || (shift && matches!(vk_code, vk::LEFT | vk::RIGHT))
            }
        }
    }

    /// キー入力を処理してアクションを返す
    pub fn handle_key(&mut self, vk_code: u16, ch: Option<char>, shift: bool) -> Action {
        match self {
            InputState::Direct => Action::Noop,
            InputState::Composing { hiragana, romaji } => {
                Self::handle_composing(hiragana, romaji, vk_code, ch, shift)
            }
            InputState::Converting {
                input,
                result,
                focused_clause,
                candidate_indices,
                boundaries,
            } => Self::handle_converting(
                input,
                result,
                focused_clause,
                candidate_indices,
                boundaries,
                vk_code,
                shift,
            ),
        }
    }

    fn handle_composing(
        hiragana: &mut String,
        romaji: &mut RomajiConverter,
        vk_code: u16,
        ch: Option<char>,
        _shift: bool,
    ) -> Action {
        match vk_code {
            vk::BACK => {
                match romaji.backspace() {
                    BackspaceResult::DeletedPending => {
                        // pending の末尾を削除した
                        let text = format!("{}{}", hiragana, romaji.pending());
                        Action::UpdateComposition(text)
                    }
                    BackspaceResult::DeleteHiragana => {
                        // ひらがなバッファの末尾1文字を削除
                        if hiragana.pop().is_some() {
                            let text = format!("{}{}", hiragana, romaji.pending());
                            if text.is_empty() {
                                Action::EndComposition(String::new())
                            } else {
                                Action::UpdateComposition(text)
                            }
                        } else {
                            Action::EndComposition(String::new())
                        }
                    }
                    BackspaceResult::Empty => Action::Noop,
                }
            }
            vk::RETURN => {
                // ひらがな + pending を確定
                let flushed = romaji.flush();
                hiragana.push_str(&flushed);
                let text = hiragana.clone();
                Action::EndComposition(text)
            }
            vk::ESCAPE => {
                romaji.reset();
                Action::EndComposition(String::new())
            }
            vk::SPACE => {
                // 変換開始
                let flushed = romaji.flush();
                hiragana.push_str(&flushed);
                if hiragana.is_empty() {
                    return Action::Noop;
                }
                let input = hiragana.clone();
                Action::StartConversion(input)
            }
            _ => {
                // 文字入力
                if let Some(c) = ch {
                    match romaji.feed(c) {
                        RomajiResult::Emit(s) => {
                            hiragana.push_str(&s);
                            let text = format!("{}{}", hiragana, romaji.pending());
                            Action::UpdateComposition(text)
                        }
                        RomajiResult::Pending => {
                            let text = format!("{}{}", hiragana, romaji.pending());
                            Action::UpdateComposition(text)
                        }
                    }
                } else {
                    Action::Noop
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_converting(
        input: &str,
        result: &ConvertResult,
        focused_clause: &mut usize,
        candidate_indices: &mut [usize],
        _boundaries: &[usize],
        vk_code: u16,
        shift: bool,
    ) -> Action {
        match vk_code {
            vk::RETURN => {
                // 確定: 各文節の選択候補を連結
                let text = Self::build_converted_text(result, candidate_indices);
                Action::EndComposition(text)
            }
            vk::ESCAPE => {
                // 取消: 元のひらがなに戻る
                Action::CancelConversion(input.to_string())
            }
            vk::SPACE | vk::DOWN => {
                // 次の候補
                if let Some(clause) = result.clauses.get(*focused_clause) {
                    let len = clause.candidates.len();
                    if len > 0 {
                        candidate_indices[*focused_clause] =
                            (candidate_indices[*focused_clause] + 1) % len;
                    }
                }
                Action::UpdateConversion
            }
            vk::UP => {
                // 前の候補
                if let Some(clause) = result.clauses.get(*focused_clause) {
                    let len = clause.candidates.len();
                    if len > 0 {
                        candidate_indices[*focused_clause] =
                            (candidate_indices[*focused_clause] + len - 1) % len;
                    }
                }
                Action::UpdateConversion
            }
            vk::RIGHT if shift => {
                // 文節伸ばす (Shift+→)
                // 実際の resize は呼び出し側で行う
                Action::UpdateConversion
            }
            vk::LEFT if shift => {
                // 文節縮める (Shift+←)
                Action::UpdateConversion
            }
            vk::RIGHT => {
                // 次の文節
                if *focused_clause + 1 < result.clauses.len() {
                    *focused_clause += 1;
                }
                Action::UpdateConversion
            }
            vk::LEFT => {
                // 前の文節
                *focused_clause = focused_clause.saturating_sub(1);
                Action::UpdateConversion
            }
            _ => Action::Noop,
        }
    }

    /// 変換結果テキストを構築 (各文節の選択候補を連結)
    fn build_converted_text(result: &ConvertResult, candidate_indices: &[usize]) -> String {
        result
            .clauses
            .iter()
            .enumerate()
            .map(|(i, clause)| {
                let idx = candidate_indices.get(i).copied().unwrap_or(0);
                clause
                    .candidates
                    .get(idx)
                    .unwrap_or(&clause.surface)
                    .as_str()
            })
            .collect()
    }

    /// 現在の表示テキスト
    pub fn display_text(&self) -> String {
        match self {
            InputState::Direct => String::new(),
            InputState::Composing { hiragana, romaji } => {
                format!("{}{}", hiragana, romaji.pending())
            }
            InputState::Converting {
                result,
                candidate_indices,
                ..
            } => Self::build_converted_text(result, candidate_indices),
        }
    }

    /// Direct ↔ Composing を切り替え
    pub fn toggle_mode(&mut self) {
        match self {
            InputState::Direct => {
                *self = InputState::Composing {
                    hiragana: String::new(),
                    romaji: RomajiConverter::new(),
                };
            }
            InputState::Composing { .. } | InputState::Converting { .. } => {
                *self = InputState::Direct;
            }
        }
    }

    /// 変換結果を設定して Converting 状態に遷移
    pub fn set_conversion_result(&mut self, input: String, result: ConvertResult) {
        let num_clauses = result.clauses.len();
        let boundaries: Vec<usize> = result
            .clauses
            .iter()
            .scan(0usize, |acc, clause| {
                *acc += clause.yomi.len();
                Some(*acc)
            })
            .collect();
        *self = InputState::Converting {
            input,
            result,
            focused_clause: 0,
            candidate_indices: vec![0; num_clauses],
            boundaries,
        };
    }

    /// Composing 状態に戻す (変換取り消し時)
    pub fn revert_to_composing(&mut self, hiragana: String) {
        *self = InputState::Composing {
            hiragana,
            romaji: RomajiConverter::new(),
        };
    }

    /// 確定後に Composing 状態に戻す
    pub fn reset_to_composing(&mut self) {
        *self = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_noop() {
        let mut state = InputState::Direct;
        assert_eq!(state.handle_key(0x41, Some('a'), false), Action::Noop);
    }

    #[test]
    fn test_toggle_mode() {
        let mut state = InputState::Direct;
        state.toggle_mode();
        assert!(matches!(state, InputState::Composing { .. }));
        state.toggle_mode();
        assert!(matches!(state, InputState::Direct));
    }

    #[test]
    fn test_composing_single_char() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        // 'k' → pending
        let action = state.handle_key(0x4B, Some('k'), false);
        assert_eq!(action, Action::UpdateComposition("k".to_string()));

        // 'a' → "か"
        let action = state.handle_key(0x41, Some('a'), false);
        assert_eq!(action, Action::UpdateComposition("か".to_string()));
    }

    #[test]
    fn test_composing_kyou() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        for (vk, ch) in [(0x4B, 'k'), (0x59, 'y'), (0x4F, 'o'), (0x55, 'u')] {
            state.handle_key(vk, Some(ch), false);
        }
        assert_eq!(state.display_text(), "きょう");
    }

    #[test]
    fn test_composing_backspace() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        // "ka" → "か"
        state.handle_key(0x4B, Some('k'), false);
        state.handle_key(0x41, Some('a'), false);
        assert_eq!(state.display_text(), "か");

        // Backspace → hiragana "か" の末尾削除 → 空
        let action = state.handle_key(vk::BACK, None, false);
        assert_eq!(action, Action::EndComposition(String::new()));
    }

    #[test]
    fn test_composing_backspace_pending() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        // "k" → pending
        state.handle_key(0x4B, Some('k'), false);
        // "y" → pending "ky"
        state.handle_key(0x59, Some('y'), false);
        assert_eq!(state.display_text(), "ky");

        // Backspace → pending "k"
        let action = state.handle_key(vk::BACK, None, false);
        assert_eq!(action, Action::UpdateComposition("k".to_string()));
    }

    #[test]
    fn test_composing_enter() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        state.handle_key(0x4B, Some('k'), false);
        state.handle_key(0x41, Some('a'), false);
        let action = state.handle_key(vk::RETURN, None, false);
        assert_eq!(action, Action::EndComposition("か".to_string()));
    }

    #[test]
    fn test_composing_escape() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        state.handle_key(0x4B, Some('k'), false);
        state.handle_key(0x41, Some('a'), false);
        let action = state.handle_key(vk::ESCAPE, None, false);
        assert_eq!(action, Action::EndComposition(String::new()));
    }

    #[test]
    fn test_composing_space_starts_conversion() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        state.handle_key(0x4B, Some('k'), false);
        state.handle_key(0x41, Some('a'), false);
        let action = state.handle_key(vk::SPACE, None, false);
        assert_eq!(action, Action::StartConversion("か".to_string()));
    }

    #[test]
    fn test_composing_space_flushes_n() {
        let mut state = InputState::Composing {
            hiragana: String::new(),
            romaji: RomajiConverter::new(),
        };
        // "kan" → "か" + pending "n"
        state.handle_key(0x4B, Some('k'), false);
        state.handle_key(0x41, Some('a'), false);
        state.handle_key(0x4E, Some('n'), false);
        // Space should flush "n" → "ん"
        let action = state.handle_key(vk::SPACE, None, false);
        assert_eq!(action, Action::StartConversion("かん".to_string()));
    }

    #[test]
    fn test_wants_key_direct() {
        let state = InputState::Direct;
        assert!(!state.wants_key(0x41, false)); // 'A'
    }

    #[test]
    fn test_wants_key_composing() {
        let state = InputState::Composing {
            hiragana: "か".to_string(),
            romaji: RomajiConverter::new(),
        };
        assert!(state.wants_key(0x41, false)); // 'A'
        assert!(state.wants_key(vk::SPACE, false));
        assert!(state.wants_key(vk::BACK, false));
        assert!(state.wants_key(vk::RETURN, false));
        assert!(state.wants_key(vk::ESCAPE, false));
    }

    #[test]
    fn test_converting_enter_commits() {
        use jamo_core::viterbi::{Clause, ConvertResult};

        let result = ConvertResult {
            segments: vec![],
            clauses: vec![Clause {
                surface: "今日".to_string(),
                yomi: "きょう".to_string(),
                candidates: vec!["今日".to_string(), "京".to_string()],
            }],
        };
        let mut state = InputState::Direct;
        state.set_conversion_result("きょう".to_string(), result);

        let action = state.handle_key(vk::RETURN, None, false);
        assert_eq!(action, Action::EndComposition("今日".to_string()));
    }

    #[test]
    fn test_converting_escape_cancels() {
        use jamo_core::viterbi::{Clause, ConvertResult};

        let result = ConvertResult {
            segments: vec![],
            clauses: vec![Clause {
                surface: "今日".to_string(),
                yomi: "きょう".to_string(),
                candidates: vec!["今日".to_string()],
            }],
        };
        let mut state = InputState::Direct;
        state.set_conversion_result("きょう".to_string(), result);

        let action = state.handle_key(vk::ESCAPE, None, false);
        assert_eq!(action, Action::CancelConversion("きょう".to_string()));
    }

    #[test]
    fn test_converting_space_next_candidate() {
        use jamo_core::viterbi::{Clause, ConvertResult};

        let result = ConvertResult {
            segments: vec![],
            clauses: vec![Clause {
                surface: "今日".to_string(),
                yomi: "きょう".to_string(),
                candidates: vec!["今日".to_string(), "京".to_string(), "教".to_string()],
            }],
        };
        let mut state = InputState::Direct;
        state.set_conversion_result("きょう".to_string(), result);

        // Space → 次の候補
        state.handle_key(vk::SPACE, None, false);
        assert_eq!(state.display_text(), "京");

        state.handle_key(vk::SPACE, None, false);
        assert_eq!(state.display_text(), "教");

        // ラップアラウンド
        state.handle_key(vk::SPACE, None, false);
        assert_eq!(state.display_text(), "今日");
    }

    #[test]
    fn test_converting_move_clause() {
        use jamo_core::viterbi::{Clause, ConvertResult};

        let result = ConvertResult {
            segments: vec![],
            clauses: vec![
                Clause {
                    surface: "今日".to_string(),
                    yomi: "きょう".to_string(),
                    candidates: vec!["今日".to_string()],
                },
                Clause {
                    surface: "は".to_string(),
                    yomi: "は".to_string(),
                    candidates: vec!["は".to_string()],
                },
            ],
        };
        let mut state = InputState::Direct;
        state.set_conversion_result("きょうは".to_string(), result);

        // → で次の文節
        state.handle_key(vk::RIGHT, None, false);
        if let InputState::Converting { focused_clause, .. } = &state {
            assert_eq!(*focused_clause, 1);
        }

        // ← で前の文節
        state.handle_key(vk::LEFT, None, false);
        if let InputState::Converting { focused_clause, .. } = &state {
            assert_eq!(*focused_clause, 0);
        }
    }
}
