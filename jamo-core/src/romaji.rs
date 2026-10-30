//! ローマ字→ひらがな変換 FSM
//!
//! ASCII キー入力を受け取り、ひらがなを出力する有限状態機械。
//! 静的ルックアップテーブルで約300のローマ字パターンを網羅。

mod table;
#[cfg(test)]
mod tests;

use table::{CARRY_TABLE, PUNCTUATION_TABLE, ROMAJI_TABLE};

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
            | 'q'
    )
}

/// 綴りに完全一致する行の (かな, 次の入力に持ち越す綴り)
fn lookup_exact(s: &str) -> Option<(&'static str, &'static str)> {
    if let Some(&(_, kana)) = ROMAJI_TABLE.iter().find(|&&(roma, _)| roma == s) {
        return Some((kana, ""));
    }
    CARRY_TABLE
        .iter()
        .find(|&&(roma, _, _)| roma == s)
        .map(|&(_, kana, rest)| (kana, rest))
}

/// s を真の接頭辞に持つ綴りがあるか
fn has_prefix_match(s: &str) -> bool {
    ROMAJI_TABLE
        .iter()
        .map(|&(roma, _)| roma)
        .chain(CARRY_TABLE.iter().map(|&(roma, _, _)| roma))
        .any(|roma| roma.starts_with(s) && roma.len() > s.len())
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
        let ch = ch.to_ascii_lowercase();
        let candidate = format!("{}{}", self.pending, ch);

        // より長い綴りがありうる間は確定しない ("n" → "na", "ww" → "www")
        if has_prefix_match(&candidate) {
            self.pending = candidate;
            return RomajiResult::Pending;
        }
        if let Some((kana, rest)) = lookup_exact(&candidate) {
            self.pending = rest.to_string();
            return RomajiResult::Emit(kana.to_string());
        }
        if self.pending.is_empty() {
            let text = PUNCTUATION_TABLE
                .iter()
                .find(|&&(punct, _)| punct == ch)
                .map(|&(_, kana)| kana.to_string())
                .unwrap_or_else(|| ch.to_string());
            return RomajiResult::Emit(text);
        }

        // 綴りが続かない: 待っていた分を確定して ch からやり直す
        let settled = self.settle_pending();
        match self.feed(ch) {
            RomajiResult::Emit(s) => RomajiResult::Emit(format!("{settled}{s}")),
            RomajiResult::Pending => RomajiResult::Emit(settled),
        }
    }

    /// 待っていた綴りを確定する。表に無い綴りは打った英字のまま返す
    fn settle_pending(&mut self) -> String {
        match lookup_exact(&self.pending) {
            Some((kana, rest)) => {
                self.pending = rest.to_string();
                kana.to_string()
            }
            None => core::mem::take(&mut self.pending),
        }
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

/// 逆変換で複数綴りがあるかなの優先綴り (テーブル先頭の綴りを使わないもの)
static REVERSE_PREFERRED: &[(&str, &str)] = &[
    ("じゃ", "ja"),
    ("じゅ", "ju"),
    ("じぇ", "je"),
    ("じょ", "jo"),
    ("い", "i"),
];

/// かな1単位 (拗音込み) の綴り。テーブルにないものは None
fn romaji_for_kana(kana: &str) -> Option<&'static str> {
    if let Some(&(_, roma)) = REVERSE_PREFERRED.iter().find(|&&(k, _)| k == kana) {
        return Some(roma);
    }
    ROMAJI_TABLE
        .iter()
        .find(|&&(_, k)| k == kana)
        .map(|&(roma, _)| roma)
}

/// ひらがな列を打鍵ローマ字に戻す (半角英字への変換用)。
/// 拗音は 2 文字単位で先に引き、っ は次の子音の重ね打ち、ん は次が母音/y/n なら
/// nn、それ以外は n。テーブルにない文字 (英数字・漢字など) はそのまま残す。
pub fn kana_to_romaji(kana: &str) -> String {
    let chars: Vec<char> = kana.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == 'っ' {
            if let Some(&next) = chars.get(i + 1)
                && let Some(first) = unit_romaji(&chars, i + 1).and_then(|s| s.chars().next())
                && next != 'っ'
                && is_romaji_consonant(first)
            {
                out.push(first);
                i += 1;
                continue;
            }
            out.push_str("xtu");
            i += 1;
            continue;
        }
        if ch == 'ん' {
            let doubled = unit_romaji(&chars, i + 1)
                .and_then(|s| s.chars().next())
                .is_some_and(|c| is_vowel(c) || matches!(c, 'y' | 'n'));
            out.push_str(if doubled { "nn" } else { "n" });
            i += 1;
            continue;
        }
        if let Some(pair) = chars.get(i..i + 2)
            && let Some(roma) = romaji_for_kana(&pair.iter().collect::<String>())
        {
            out.push_str(roma);
            i += 2;
            continue;
        }
        match romaji_for_kana(&ch.to_string()) {
            Some(roma) => out.push_str(roma),
            None => out.push(punct_to_ascii(ch).unwrap_or(ch)),
        }
        i += 1;
    }
    out
}

/// 和文句読点 (。、・「」ー！？〜) を打鍵した ASCII 記号に戻す
pub fn punct_to_ascii(ch: char) -> Option<char> {
    PUNCTUATION_TABLE
        .iter()
        .find(|&&(_, hira)| hira.starts_with(ch))
        .map(|&(p, _)| p)
}

/// 位置 i から始まるかな1単位 (拗音優先) の綴り
fn unit_romaji(chars: &[char], i: usize) -> Option<&'static str> {
    if let Some(pair) = chars.get(i..i + 2)
        && let Some(roma) = romaji_for_kana(&pair.iter().collect::<String>())
    {
        return Some(roma);
    }
    chars.get(i).and_then(|c| romaji_for_kana(&c.to_string()))
}

impl Default for RomajiConverter {
    fn default() -> Self {
        Self::new()
    }
}
