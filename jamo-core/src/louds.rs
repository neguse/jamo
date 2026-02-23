//! LOUDS (Level-Order Unary Degree Sequence) trie
//!
//! ビットベクトルとラベル配列で trie を簡潔に表現する。
//! バイト単位の trie で UTF-8 文字列の共通接頭辞検索を行う。

use crate::bitvec::BitVec;
use crate::dict::Entry;

/// LOUDS trie
pub struct LoudsTrie {
    /// LOUDS ビット列: ノードの子の数を unary 符号で表現
    /// スーパールートの "10" から始まる
    bits: BitVec,
    /// 各ノードのエッジラベル (バイト値)
    /// インデックスはノード番号 (0=スーパールート, 1=ルート, 2..=子ノード)
    labels: Vec<u8>,
    /// ノードがキーの終端なら Some(エントリスライスの開始,終了インデックス)
    /// そうでなければ None
    terminal: Vec<Option<(u32, u32)>>,
    /// エントリプール
    entries: Vec<Entry>,
}

/// 構築用の中間 trie ノード
struct BuildNode {
    label: u8,
    children: Vec<usize>, // BuildNode のインデックス
    terminal: Option<(u32, u32)>,
}

impl LoudsTrie {
    /// ソート済みの (キー, エントリ一覧) から LOUDS trie を構築
    pub fn build(sorted_keys: &[(&[u8], &[Entry])]) -> Self {
        // エントリプールを構築
        let mut entries = Vec::new();
        let mut key_entries: Vec<(&[u8], u32, u32)> = Vec::new();
        for &(key, ents) in sorted_keys {
            let start = entries.len() as u32;
            entries.extend_from_slice(ents);
            let end = entries.len() as u32;
            key_entries.push((key, start, end));
        }

        // 中間 trie 構築
        let mut nodes: Vec<BuildNode> = Vec::new();
        // ルートノード (label は使わない)
        nodes.push(BuildNode {
            label: 0,
            children: Vec::new(),
            terminal: None,
        });

        for &(key, start, end) in &key_entries {
            let mut current = 0; // ルートから開始
            for &byte in key {
                let found = nodes[current]
                    .children
                    .iter()
                    .find(|&&child_idx| nodes[child_idx].label == byte)
                    .copied();
                match found {
                    Some(child_idx) => current = child_idx,
                    None => {
                        let new_idx = nodes.len();
                        nodes.push(BuildNode {
                            label: byte,
                            children: Vec::new(),
                            terminal: None,
                        });
                        nodes[current].children.push(new_idx);
                        current = new_idx;
                    }
                }
            }
            nodes[current].terminal = Some((start, end));
        }

        // BFS で LOUDS ビット列を生成
        let mut bits = Vec::new();
        let mut labels = Vec::new();
        let mut terminal = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        // スーパールート: 子1つ (ルート) → "10"
        bits.push(true);
        bits.push(false);
        labels.push(0); // スーパールートのラベル
        terminal.push(None);
        queue.push_back(0); // ルートノード

        while let Some(node_idx) = queue.pop_front() {
            let node = &nodes[node_idx];
            labels.push(node.label);
            terminal.push(node.terminal);

            // 子のぶんだけ 1 を並べ、最後に 0
            for &child_idx in &node.children {
                bits.push(true);
                queue.push_back(child_idx);
            }
            bits.push(false);
        }

        Self {
            bits: BitVec::from_bits(&bits),
            labels,
            terminal,
            entries,
        }
    }

    /// ノード node のビット領域の開始位置 (子の符号化が始まる位置)
    ///
    /// LOUDS ビット列の構造:
    ///   node 0 のビット: [0 .. select0(0)]
    ///   node i のビット: [select0(i-1)+1 .. select0(i)]
    /// ノードのビット領域は "1...10" (子の数だけ1、最後に0) の形。
    fn node_bit_start(&self, node: usize) -> usize {
        if node == 0 {
            0
        } else {
            self.bits.select0(node - 1).unwrap() + 1
        }
    }

    /// ビット位置 bit_pos にある1ビットに対応する子ノードの番号
    ///
    /// LOUDS では各1ビットがノード (super root 以外) と1対1対応する。
    /// bit_pos にある k 番目 (0-indexed) の1ビット → ノード k+1。
    fn node_from_bit(&self, bit_pos: usize) -> usize {
        self.bits.rank1(bit_pos) + 1
    }

    /// ノード node の子を列挙
    #[cfg(test)]
    fn children(&self, node: usize) -> Vec<(u8, usize)> {
        let start = self.node_bit_start(node);
        let mut result = Vec::new();
        let mut pos = start;
        while pos < self.bits.len() && self.bits.get(pos) {
            let child_node = self.node_from_bit(pos);
            result.push((self.labels[child_node], child_node));
            pos += 1;
        }
        result
    }

    /// ノード node の子のうちラベルが label のものを返す
    fn find_child(&self, node: usize, label: u8) -> Option<usize> {
        let start = self.node_bit_start(node);
        let mut pos = start;
        while pos < self.bits.len() && self.bits.get(pos) {
            let child_node = self.node_from_bit(pos);
            if self.labels[child_node] == label {
                return Some(child_node);
            }
            pos += 1;
        }
        None
    }

    /// 共通接頭辞検索: 入力の先頭から一致するすべてのキーのエントリを返す
    /// 戻り値: (マッチしたバイト長, エントリ一覧)
    pub fn common_prefix_search(&self, input: &str) -> Vec<(usize, Vec<Entry>)> {
        let bytes = input.as_bytes();
        let mut results = Vec::new();
        let node = 1; // ルートノード (ノード番号1, スーパールートの子)

        // ルートが存在しない場合 (空の trie ではありえないが念のため)
        if node >= self.labels.len() {
            return results;
        }

        // ルート自体がキー終端の場合 (空文字列キー)
        if let Some((start, end)) = self.terminal[node] {
            results.push((0, self.entries[start as usize..end as usize].to_vec()));
        }

        let mut current = node;
        for (i, &byte) in bytes.iter().enumerate() {
            match self.find_child(current, byte) {
                Some(child) => {
                    current = child;
                    if let Some((start, end)) = self.terminal[current] {
                        results.push((i + 1, self.entries[start as usize..end as usize].to_vec()));
                    }
                }
                None => break,
            }
        }

        results
    }

    /// ノード数
    pub fn num_nodes(&self) -> usize {
        self.labels.len()
    }

    /// ビット数
    pub fn num_bits(&self) -> usize {
        self.bits.len()
    }

    /// シリアライズ用アクセサ
    pub fn bits(&self) -> &BitVec {
        &self.bits
    }

    /// シリアライズ用アクセサ
    pub fn labels(&self) -> &[u8] {
        &self.labels
    }

    /// シリアライズ用アクセサ
    pub fn terminal(&self) -> &[Option<(u32, u32)>] {
        &self.terminal
    }

    /// シリアライズ用アクセサ
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// デシリアライズ用: 各フィールドから直接構築
    pub fn from_parts(
        bits: BitVec,
        labels: Vec<u8>,
        terminal: Vec<Option<(u32, u32)>>,
        entries: Vec<Entry>,
    ) -> Self {
        Self {
            bits,
            labels,
            terminal,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(surface: &str, cost: i16) -> Entry {
        Entry {
            surface: surface.to_string(),
            left_id: 0,
            right_id: 0,
            cost,
        }
    }

    #[test]
    fn test_empty_trie() {
        let trie = LoudsTrie::build(&[]);
        let results = trie.common_prefix_search("きょう");
        assert!(results.is_empty());
    }

    #[test]
    fn test_single_key() {
        let entries = vec![make_entry("今日", -3000)];
        let keys: Vec<(&[u8], &[Entry])> = vec![("きょう".as_bytes(), &entries)];
        let trie = LoudsTrie::build(&keys);

        let results = trie.common_prefix_search("きょう");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "きょう".len());
        assert_eq!(results[0].1[0].surface, "今日");
    }

    #[test]
    fn test_prefix_search() {
        let e_ki = vec![make_entry("木", -2500), make_entry("気", -2000)];
        let e_kyou = vec![make_entry("今日", -3000), make_entry("京", -2000)];

        let mut keys: Vec<(&[u8], &[Entry])> =
            vec![("き".as_bytes(), &e_ki), ("きょう".as_bytes(), &e_kyou)];
        keys.sort_by_key(|(k, _)| k.to_vec());

        let trie = LoudsTrie::build(&keys);

        // "きょうは" で検索: "き" と "きょう" がマッチ
        let results = trie.common_prefix_search("きょうは");
        let lengths: Vec<usize> = results.iter().map(|(len, _)| *len).collect();
        assert!(
            lengths.contains(&"き".len()),
            "「き」がマッチするはず: {lengths:?}"
        );
        assert!(
            lengths.contains(&"きょう".len()),
            "「きょう」がマッチするはず: {lengths:?}"
        );
    }

    #[test]
    fn test_multiple_candidates() {
        let e_kyou = vec![make_entry("今日", -3000), make_entry("京", -2000)];
        let keys: Vec<(&[u8], &[Entry])> = vec![("きょう".as_bytes(), &e_kyou)];
        let trie = LoudsTrie::build(&keys);

        let results = trie.common_prefix_search("きょう");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.len(), 2);
        let surfaces: Vec<&str> = results[0].1.iter().map(|e| e.surface.as_str()).collect();
        assert!(surfaces.contains(&"今日"));
        assert!(surfaces.contains(&"京"));
    }

    #[test]
    fn test_no_match() {
        let entries = vec![make_entry("今日", -3000)];
        let keys: Vec<(&[u8], &[Entry])> = vec![("きょう".as_bytes(), &entries)];
        let trie = LoudsTrie::build(&keys);

        let results = trie.common_prefix_search("zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_children_navigation() {
        let e_a = vec![make_entry("a", -1000)];
        let e_ab = vec![make_entry("ab", -1000)];
        let e_ac = vec![make_entry("ac", -1000)];
        let e_b = vec![make_entry("b", -1000)];

        let mut keys: Vec<(&[u8], &[Entry])> =
            vec![(b"a", &e_a), (b"ab", &e_ab), (b"ac", &e_ac), (b"b", &e_b)];
        keys.sort_by_key(|(k, _)| k.to_vec());

        let trie = LoudsTrie::build(&keys);

        // ルート (ノード1) の子は 'a' と 'b'
        let root_children = trie.children(1);
        assert_eq!(root_children.len(), 2);
        assert_eq!(root_children[0].0, b'a');
        assert_eq!(root_children[1].0, b'b');

        // 'a' ノードの子は 'b' と 'c'
        let a_node = root_children[0].1;
        let a_children = trie.children(a_node);
        assert_eq!(a_children.len(), 2);
        assert_eq!(a_children[0].0, b'b');
        assert_eq!(a_children[1].0, b'c');
    }

    #[test]
    fn test_hiragana_trie() {
        // 実際のかな漢字変換で使うパターン
        let e_ten = vec![make_entry("天", -2000)];
        let e_tenki = vec![make_entry("天気", -3500)];
        let e_te = vec![make_entry("て", -1000)];

        let mut keys: Vec<(&[u8], &[Entry])> = vec![
            ("て".as_bytes(), &e_te),
            ("てん".as_bytes(), &e_ten),
            ("てんき".as_bytes(), &e_tenki),
        ];
        keys.sort_by_key(|(k, _)| k.to_vec());

        let trie = LoudsTrie::build(&keys);

        let results = trie.common_prefix_search("てんきですね");
        let lengths: Vec<usize> = results.iter().map(|(len, _)| *len).collect();
        assert!(lengths.contains(&"て".len()));
        assert!(lengths.contains(&"てん".len()));
        assert!(lengths.contains(&"てんき".len()));
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_num_nodes() {
        // "a", "ab" → スーパールート→ルート→'a'→'b' = 4 ノード
        let e_a = vec![make_entry("a", -1000)];
        let e_ab = vec![make_entry("ab", -1000)];
        let keys: Vec<(&[u8], &[Entry])> = vec![(b"a", &e_a), (b"ab", &e_ab)];
        let trie = LoudsTrie::build(&keys);

        // スーパールート + ルート + 'a' + 'b' = 4
        assert_eq!(trie.num_nodes(), 4);
    }

    #[test]
    fn test_many_keys() {
        let all_entries: Vec<Vec<Entry>> = (0..100)
            .map(|i| vec![make_entry(&format!("word{i}"), -(i as i16))])
            .collect();
        let key_strings: Vec<String> = (0..100).map(|i| format!("key{i:03}")).collect();
        let mut keys: Vec<(&[u8], &[Entry])> = key_strings
            .iter()
            .enumerate()
            .map(|(i, k)| (k.as_bytes(), all_entries[i].as_slice()))
            .collect();
        keys.sort_by_key(|(k, _)| k.to_vec());

        let trie = LoudsTrie::build(&keys);

        let results = trie.common_prefix_search("key000");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1[0].surface, "word0");
    }
}
