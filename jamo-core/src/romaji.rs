//! ローマ字→ひらがな変換 FSM
//!
//! ASCII キー入力を受け取り、ひらがなを出力する有限状態機械。
//! 静的ルックアップテーブルで約200のローマ字パターンを網羅。

mod table;
#[cfg(test)]
mod tests;

use table::{PUNCTUATION_TABLE, ROMAJI_TABLE};

/// 母音かどうか
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'i' | 'u' | 'e' | 'o')
}

/// ローマ字の子音として有効な文字かどうか
fn is_romaji_consonant(c: char) -> bool {
    matches!(
        c,
        'k' | 's'
            | 't'
            | 'n'
            | 'h'
            | 'm'
            | 'y'
            | 'r'
            | 'w'
            | 'g'
            | 'z'
            | 'd'
            | 'b'
            | 'p'
            | 'c'
            | 'f'
            | 'j'
            | 'v'
            | 'x'
            | 'l'
    )
}

/// 促音を生成する子音の重複パターンかどうか
fn is_double_consonant(c: char) -> bool {
    matches!(
        c,
        'k' | 's'
            | 't'
            | 'h'
            | 'm'
            | 'r'
            | 'w'
            | 'g'
            | 'z'
            | 'd'
            | 'b'
            | 'p'
            | 'c'
            | 'f'
            | 'j'
            | 'v'
    )
}

/// テーブルからマッチを探す
/// 完全一致があれば Some((ひらがな, 消費文字数))、なければ None
fn lookup_exact(s: &str) -> Option<(&'static str, usize)> {
    for &(roma, hira) in ROMAJI_TABLE {
        if s == roma {
            return Some((hira, roma.len()));
        }
    }
    None
}

/// テーブルにプレフィックスマッチするパターンがあるか
fn has_prefix_match(s: &str) -> bool {
    for &(roma, _) in ROMAJI_TABLE {
        if roma.starts_with(s) && roma.len() > s.len() {
            return true;
        }
    }
    false
}

/// ローマ字変換結果
#[derive(Debug, Clone, PartialEq)]
pub enum RomajiResult {
    /// ひらがな確定 (複数文字の場合あり: 促音 "っか" など)
    Emit(String),
    /// 入力待ち (まだ確定できない)
    Pending,
}

/// Backspace 処理結果
#[derive(Debug, Clone, PartialEq)]
pub enum BackspaceResult {
    /// pending の末尾を削除した
    DeletedPending,
    /// pending が空 → 上位レイヤーがひらがなを削除すべき
    DeleteHiragana,
    /// 何もない (Direct 状態など)
    Empty,
}

/// ローマ字→ひらがな変換器
#[derive(Debug, Clone)]
pub struct RomajiConverter {
    pending: String,
}

impl RomajiConverter {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// 1文字入力し、ひらがな確定 or 待機を返す
    pub fn feed(&mut self, ch: char) -> RomajiResult {
        // "n'" → ん (アポストロフィは ん の区切りとして消費する)
        if self.pending == "n" && ch == '\'' {
            self.pending.clear();
            return RomajiResult::Emit("ん".to_string());
        }

        // 句読点・ローマ字でない文字: 未確定分を flush してから続ける
        let ch_lower = ch.to_ascii_lowercase();
        if !ch_lower.is_ascii_alphabetic() {
            let flushed = self.flush();
            let text = PUNCTUATION_TABLE
                .iter()
                .find(|&&(punct, _)| punct == ch)
                .map(|&(_, hira)| hira.to_string())
                .unwrap_or_else(|| ch.to_string());
            return RomajiResult::Emit(format!("{flushed}{text}"));
        }

        let candidate = format!("{}{}", self.pending, ch_lower);

        // "nn" → "ん" (即座に確定)
        if self.pending == "n" && ch_lower == 'n' {
            self.pending.clear();
            return RomajiResult::Emit("ん".to_string());
        }

        // "n" の特殊処理: n の後に母音・y 以外の子音が来たら "ん" + 新しい子音
        if self.pending == "n" && ch_lower != 'y' && !is_vowel(ch_lower) {
            if is_romaji_consonant(ch_lower) {
                self.pending = ch_lower.to_string();
                return RomajiResult::Emit("ん".to_string());
            }
            // ローマ字の子音でもない文字
            self.pending.clear();
            return RomajiResult::Emit(format!("ん{ch_lower}"));
        }

        // "m" + b/p → "ん" (shimbun → しんぶん)
        if self.pending == "m" && matches!(ch_lower, 'b' | 'p') {
            self.pending = ch_lower.to_string();
            return RomajiResult::Emit("ん".to_string());
        }

        // 完全一致チェック
        if let Some((hira, _)) = lookup_exact(&candidate) {
            // "n" 単体マッチは特殊: まだ確定しない (na, ni 等の可能性)
            if candidate == "n" {
                self.pending = candidate;
                return RomajiResult::Pending;
            }
            self.pending.clear();
            return RomajiResult::Emit(hira.to_string());
        }

        // プレフィックスマッチ (まだ長いパターンがありえる)
        if has_prefix_match(&candidate) {
            self.pending = candidate;
            return RomajiResult::Pending;
        }

        // 促音チェック: 同じ子音の連続 (nn は除外: "ん" で処理済み)
        if candidate.len() == 2
            && ch_lower != 'n'
            && is_double_consonant(ch_lower)
            && self.pending.len() == 1
            && self.pending.starts_with(ch_lower)
        {
            self.pending = ch_lower.to_string();
            return RomajiResult::Emit("っ".to_string());
        }

        // マッチしない → pending をリセットして再試行
        if !self.pending.is_empty() {
            // pending を捨てて、新しい文字から再開
            self.pending.clear();
            return self.feed(ch);
        }

        // 単独文字でもマッチしない
        RomajiResult::Pending
    }

    /// Backspace 処理
    pub fn backspace(&mut self) -> BackspaceResult {
        if self.pending.is_empty() {
            BackspaceResult::DeleteHiragana
        } else {
            self.pending.pop();
            BackspaceResult::DeletedPending
        }
    }

    /// pending を強制確定する。"n" は「ん」、未完成の子音は打った英字のまま返す
    /// (黙って消すと入力が失われる)
    pub fn flush(&mut self) -> String {
        if self.pending == "n" {
            self.pending.clear();
            "ん".to_string()
        } else {
            core::mem::take(&mut self.pending)
        }
    }

    /// 現在の未確定ローマ字
    pub fn pending(&self) -> &str {
        &self.pending
    }

    /// 状態クリア
    pub fn reset(&mut self) {
        self.pending.clear();
    }
}

impl Default for RomajiConverter {
    fn default() -> Self {
        Self::new()
    }
}
