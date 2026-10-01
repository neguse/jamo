//! v3 辞書バイナリのビルダー
//!
//! 入力は (ひらがな読み, エントリ一覧) のリストと接続行列。出力は
//! `BinDict` がそのまま借用参照できるバイト列。

use std::collections::HashMap;
use std::io;

use super::charcode::{
    READING_TABLE_MAX, ReadingCoder, SURFACE_TABLE_MAX, SurfaceCoder, frequency_table, write_table,
};
use super::format::{
    KIND_HIRAGANA, KIND_KATAKANA, KIND_POOL, MAX_COMBOS, MAX_POS, SectionWriter, invalid,
};
use crate::bitvec::BitVec;
use crate::dict::Entry;
use crate::kana::to_katakana;

/// ビルド入力
pub struct BuildInput<'a> {
    /// 読み (ひらがな正規化済み) → エントリ一覧。順序は問わない
    pub keys: &'a [(String, Vec<Entry>)],
    /// 接続コスト行列 [right_id * num_pos + left_id]
    pub matrix: &'a [i16],
    pub num_pos: usize,
}

struct BuildNode {
    label: u8,
    children: Vec<usize>,
    /// 終端ならキーのインデックス
    key: Option<usize>,
}

/// trie を構築し、BFS 順に (LOUDS, labels, terminal, 終端のキー順) を返す
fn build_trie(encoded: &[Vec<u8>]) -> (BitVec, Vec<u8>, BitVec, Vec<usize>) {
    let mut nodes = vec![BuildNode {
        label: 0,
        children: Vec::new(),
        key: None,
    }];
    let mut order: Vec<usize> = (0..encoded.len()).collect();
    order.sort_by(|&a, &b| encoded[a].cmp(&encoded[b]));
    for key_idx in order {
        let mut current = 0;
        for &byte in &encoded[key_idx] {
            let found = nodes[current]
                .children
                .iter()
                .copied()
                .find(|&c| nodes[c].label == byte);
            current = match found {
                Some(c) => c,
                None => {
                    let idx = nodes.len();
                    nodes.push(BuildNode {
                        label: byte,
                        children: Vec::new(),
                        key: None,
                    });
                    nodes[current].children.push(idx);
                    idx
                }
            };
        }
        nodes[current].key = Some(key_idx);
    }

    let mut louds = BitVec::new();
    let mut labels = Vec::with_capacity(nodes.len() + 1);
    let mut terminal = BitVec::new();
    let mut terminal_keys = Vec::new();
    louds.push(true);
    louds.push(false);
    labels.push(0);
    terminal.push(false);
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(0usize);
    while let Some(idx) = queue.pop_front() {
        let node = &nodes[idx];
        labels.push(node.label);
        terminal.push(node.key.is_some());
        if let Some(k) = node.key {
            terminal_keys.push(k);
        }
        for &c in &node.children {
            louds.push(true);
            queue.push_back(c);
        }
        louds.push(false);
    }
    (louds, labels, terminal, terminal_keys)
}

/// 行ごとの最頻値をデフォルトにし、例外だけをビット列 + コストで持つ
fn build_matrix(matrix: &[i16], num_pos: usize) -> io::Result<Vec<u8>> {
    if num_pos == 0 || num_pos > MAX_POS || matrix.len() != num_pos * num_pos {
        return Err(invalid(format!(
            "matrix size mismatch: num_pos={num_pos}, len={}",
            matrix.len()
        )));
    }
    let mut defaults = Vec::with_capacity(num_pos);
    let mut exc = BitVec::new();
    let mut costs = Vec::new();
    let mut freq: HashMap<i16, u32> = HashMap::new();
    for r in 0..num_pos {
        let row = &matrix[r * num_pos..(r + 1) * num_pos];
        freq.clear();
        for &v in row {
            *freq.entry(v).or_default() += 1;
        }
        let default = freq
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(&v, _)| v)
            .unwrap_or(0);
        defaults.push(default);
        for &v in row {
            let is_exc = v != default;
            exc.push(is_exc);
            if is_exc {
                costs.push(v);
            }
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&(num_pos as u32).to_le_bytes());
    for d in defaults {
        out.extend_from_slice(&d.to_le_bytes());
    }
    exc.append_rank_only(&mut out);
    for c in costs {
        out.extend_from_slice(&c.to_le_bytes());
    }
    Ok(out)
}

/// 終端ノード順に並べたトークン列と付随表
struct TokenStream<'a> {
    tokens: Vec<u8>,
    entry_bounds: BitVec,
    nonident: BitVec,
    combos: Vec<(u16, u16, u8)>,
    /// 表層プールに入る表層 (id 順、重複なし)
    surfaces: Vec<&'a str>,
    surface_ids: Vec<u8>,
    vocab: usize,
}

/// 終端ノードの BFS 順 (`terminal_keys`) にエントリを並べてトークン化する
fn build_tokens<'a>(
    keys: &'a [(String, Vec<Entry>)],
    terminal_keys: &[usize],
) -> io::Result<TokenStream<'a>> {
    let mut s = TokenStream {
        tokens: Vec::new(),
        entry_bounds: BitVec::new(),
        nonident: BitVec::new(),
        combos: Vec::new(),
        surfaces: Vec::new(),
        surface_ids: Vec::new(),
        vocab: 0,
    };
    let mut combo_index: HashMap<(u16, u16, u8), u16> = HashMap::new();
    let mut surface_index: HashMap<&str, u32> = HashMap::new();
    for &k in terminal_keys {
        let (reading, entries) = &keys[k];
        let katakana = to_katakana(reading);
        for (i, e) in entries.iter().enumerate() {
            let combo_key = (e.left_id, e.right_id, e.pos_type);
            let combo = match combo_index.get(&combo_key) {
                Some(&c) => c,
                None => {
                    if s.combos.len() >= MAX_COMBOS {
                        return Err(invalid(format!("too many id combos (> {MAX_COMBOS})")));
                    }
                    let c = s.combos.len() as u16;
                    s.combos.push(combo_key);
                    combo_index.insert(combo_key, c);
                    c
                }
            };
            let kind = if e.surface == *reading {
                KIND_HIRAGANA
            } else if e.surface == katakana {
                KIND_KATAKANA
            } else {
                KIND_POOL
            };
            if kind == KIND_POOL {
                if e.surface.is_empty() {
                    return Err(invalid(format!("empty surface for reading {reading:?}")));
                }
                let id = match surface_index.get(e.surface.as_str()) {
                    Some(&id) => id,
                    None => {
                        let id = s.surfaces.len() as u32;
                        s.surfaces.push(e.surface.as_str());
                        surface_index.insert(e.surface.as_str(), id);
                        id
                    }
                };
                s.surface_ids.extend_from_slice(&id.to_le_bytes()[..3]);
            }
            s.tokens
                .extend_from_slice(&((kind << 14) | combo).to_le_bytes());
            s.tokens.extend_from_slice(&e.cost.to_le_bytes());
            s.entry_bounds.push(i == 0);
            s.nonident.push(kind == KIND_POOL);
            s.vocab += 1;
        }
    }
    if s.surfaces.len() >= 1 << 24 {
        return Err(invalid("too many surfaces (> 2^24)"));
    }
    Ok(s)
}

/// 表層を可変長文字符号化してプールに連結する
fn build_surface_pool(surfaces: &[&str]) -> io::Result<(SurfaceCoder, Vec<u8>, BitVec)> {
    let surface_coder =
        SurfaceCoder::new(frequency_table(surfaces.iter().copied(), SURFACE_TABLE_MAX))
            .map_err(invalid)?;
    let mut pool = Vec::new();
    let mut surface_bounds = BitVec::new();
    for s in surfaces {
        let start = pool.len();
        surface_coder.encode(s, &mut pool).map_err(invalid)?;
        for i in start..pool.len() {
            surface_bounds.push(i == start);
        }
    }
    Ok((surface_coder, pool, surface_bounds))
}

/// v3 辞書バイナリを構築する
pub fn build(input: BuildInput<'_>) -> io::Result<Vec<u8>> {
    let keys = input.keys;
    let reading_coder = ReadingCoder::new(frequency_table(
        keys.iter().map(|(k, _)| k.as_str()),
        READING_TABLE_MAX,
    ));
    let encoded: Vec<Vec<u8>> = keys.iter().map(|(k, _)| reading_coder.encode(k)).collect();
    let (louds, labels, terminal, terminal_keys) = build_trie(&encoded);

    let stream = build_tokens(keys, &terminal_keys)?;
    let (surface_coder, pool, surface_bounds) = build_surface_pool(&stream.surfaces)?;

    let mut w = SectionWriter::new();
    let mut tmp = Vec::new();
    write_table(reading_coder.chars(), &mut tmp);
    w.section(&tmp);
    tmp.clear();
    louds.append_to(&mut tmp);
    w.section(&tmp);
    w.section(&labels);
    tmp.clear();
    terminal.append_to(&mut tmp);
    w.section(&tmp);
    tmp.clear();
    stream.entry_bounds.append_to(&mut tmp);
    w.section(&tmp);
    w.section(&stream.tokens);
    tmp.clear();
    for (l, r, p) in &stream.combos {
        tmp.extend_from_slice(&l.to_le_bytes());
        tmp.extend_from_slice(&r.to_le_bytes());
        tmp.push(*p);
    }
    w.section(&tmp);
    tmp.clear();
    stream.nonident.append_to(&mut tmp);
    w.section(&tmp);
    w.section(&stream.surface_ids);
    tmp.clear();
    write_table(surface_coder.chars(), &mut tmp);
    w.section(&tmp);
    w.section(&pool);
    tmp.clear();
    surface_bounds.append_to(&mut tmp);
    w.section(&tmp);
    w.section(&build_matrix(input.matrix, input.num_pos)?);
    tmp.clear();
    tmp.extend_from_slice(&(stream.vocab as u32).to_le_bytes());
    tmp.extend_from_slice(&(keys.len() as u32).to_le_bytes());
    w.section(&tmp);
    w.finish()
}
