//! 逆変換 (表層 → 読み)
//!
//! 漢字かな交じりの文字列に読みを付ける。かな漢字変換と同じ辞書・単語コスト・
//! 接続コスト・文節境界ペナルティを使い、入力の部分文字列と表層が一致する語で
//! ラティスを作って、Viterbi で最良の語の並びを選ぶ (mozc の再変換と同じ考え方)。
//! 同じ字でも前後の語で読みが決まる (「触らない間は」の 間 → あいだ)。

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::dict::{Dictionary, Entry};
use crate::kana::{is_kana, to_hiragana};
use crate::lattice::{Lattice, Node};
use crate::viterbi::{ConnectionCost, best_path};

/// 1 語として引く部分文字列の最大文字数
const MAX_WORD_CHARS: usize = 24;

/// 逆変換の 1 語
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ipc", derive(serde::Serialize, serde::Deserialize))]
pub struct Word {
    /// 表層 (入力の部分文字列)
    pub surface: String,
    /// 読み (ひらがな)。辞書に無い字は、かなならひらがなにし、ほかはその字のまま
    pub reading: String,
}

/// `text` を語に分けて読みを付ける。語の表層をつなぐと `text` になる
pub fn reading(text: &str, dict: &dyn Dictionary, conn: &dyn ConnectionCost) -> Vec<Word> {
    readings(&[text], dict, conn).pop().unwrap_or_default()
}

/// `texts` のそれぞれに `reading` をかける。辞書を表層で引くのは全部まとめて 1 回にする
/// (表層からの索引を持たない辞書では、引くたびに辞書全体を読むため)
pub fn readings(
    texts: &[&str],
    dict: &dyn Dictionary,
    conn: &dyn ConnectionCost,
) -> Vec<Vec<Word>> {
    let mut spans: Vec<&str> = Vec::new();
    for text in texts {
        let starts = char_starts(text);
        for (i, &a) in starts.iter().enumerate() {
            for &b in starts.iter().skip(i + 1).take(MAX_WORD_CHARS) {
                spans.push(&text[a..b]);
            }
        }
    }
    spans.sort_unstable();
    spans.dedup();
    let mut by_surface: HashMap<String, Vec<(String, Entry)>> = HashMap::new();
    for (reading, entry) in dict.reverse_lookup(&spans) {
        by_surface
            .entry(entry.surface.clone())
            .or_default()
            .push((reading, entry));
    }
    // 同じコストの語の並びが複数あっても結果が辞書の走査順に左右されないよう、決まった順に並べる
    for words in by_surface.values_mut() {
        words.sort_by(|a, b| (a.1.cost, &a.0).cmp(&(b.1.cost, &b.0)));
    }
    texts
        .iter()
        .map(|text| best_words(text, &by_surface, dict, conn))
        .collect()
}

/// 文字の先頭のバイト位置と、末尾 (text.len())
fn char_starts(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect()
}

/// 表層ごとの (読み, エントリ) から `text` のラティスを作り、最良の語の並びを返す
fn best_words(
    text: &str,
    by_surface: &HashMap<String, Vec<(String, Entry)>>,
    dict: &dyn Dictionary,
    conn: &dyn ConnectionCost,
) -> Vec<Word> {
    let starts = char_starts(text);
    let mut nodes = Vec::new();
    let mut readings = Vec::new();
    for (i, &a) in starts.iter().enumerate() {
        let Some(ch) = text[a..].chars().next() else {
            break;
        };
        for &b in starts.iter().skip(i + 1).take(MAX_WORD_CHARS) {
            for (reading, e) in by_surface.get(&text[a..b]).into_iter().flatten() {
                nodes.push(Node::word(
                    a,
                    b,
                    e.surface.clone(),
                    e.left_id,
                    e.right_id,
                    e.cost as i64,
                    e.pos_type,
                ));
                readings.push(reading.clone());
            }
        }
        nodes.push(Node::unknown(a, ch));
        let c = ch.to_string();
        readings.push(if is_kana(&c) { to_hiragana(&c) } else { c });
    }

    // from_words は BOS を先頭に置くので、ノード番号は語の番号 + 1
    let mut lattice = Lattice::from_words(text.len(), nodes, dict);
    best_path(&mut lattice, conn)
        .into_iter()
        .map(|idx| Word {
            surface: lattice.nodes[idx].surface.clone(),
            reading: readings[idx - 1].clone(),
        })
        .collect()
}
