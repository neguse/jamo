//! 変換学習 (読み→確定表層の LRU、直前文節を文脈に持つ)
//!
//! commit 時に「直前文節がこの表層のとき、この読みをこの表層で確定した」を
//! 記録し、次回変換時に同じ文脈ならその表層を、なければ文脈を問わず最新の
//! 表層を文節候補の先頭に持ってくる。文脈は「雨が|降って」のような単文節
//! 学習では直らない連語を覚えるためのもの。容量上限つきで古いものから消える。
//! 永続化はホスト提供のパスへの TSV (文脈, 読み, 表層。recency 順、先頭が最新)。

use std::fs;
use std::io;
use std::path::Path;

const DEFAULT_CAPACITY: usize = 1024;

/// 学習エントリ
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    /// 直前文節の確定表層 (文頭は空)
    context: String,
    yomi: String,
    surface: String,
}

/// 読み→表層の学習ストア (recency 順、先頭が最新)
#[derive(Debug, Clone)]
pub struct LearningStore {
    entries: Vec<Entry>,
    capacity: usize,
}

impl Default for LearningStore {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl LearningStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// 確定を記録する。同じ (文脈, 読み) の既存エントリは上書きされ、最新になる
    pub fn record(&mut self, context: &str, yomi: &str, surface: &str) {
        if yomi.is_empty() || surface.is_empty() {
            return;
        }
        self.entries
            .retain(|e| !(e.yomi == yomi && e.context == context));
        self.entries.insert(
            0,
            Entry {
                context: context.to_string(),
                yomi: yomi.to_string(),
                surface: surface.to_string(),
            },
        );
        self.entries.truncate(self.capacity);
    }

    /// 読みに対する学習済み表層。同じ文脈のものを優先し、なければ文脈を問わず最新
    pub fn get(&self, context: &str, yomi: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.yomi == yomi && e.context == context)
            .or_else(|| self.entries.iter().find(|e| e.yomi == yomi))
            .map(|e| e.surface.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// TSV 形式にシリアライズ (文脈, 読み, 表層。recency 順)
    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&e.context);
            out.push('\t');
            out.push_str(&e.yomi);
            out.push('\t');
            out.push_str(&e.surface);
            out.push('\n');
        }
        out
    }

    /// TSV から復元。不正な行は読み飛ばす。
    /// 2 列 (読み, 表層) の行は文脈なしとして受け入れる
    pub fn from_tsv(tsv: &str, capacity: usize) -> Self {
        let mut store = Self::new(capacity);
        for line in tsv.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            let (context, yomi, surface) = match cols.as_slice() {
                [yomi, surface] => ("", *yomi, *surface),
                [context, yomi, surface] => (*context, *yomi, *surface),
                _ => continue,
            };
            if yomi.is_empty() || surface.is_empty() || store.entries.len() >= store.capacity {
                continue;
            }
            store.entries.push(Entry {
                context: context.to_string(),
                yomi: yomi.to_string(),
                surface: surface.to_string(),
            });
        }
        store
    }

    /// ファイルへ保存
    pub fn save(&self, path: &Path) -> io::Result<()> {
        fs::write(path, self.to_tsv())
    }

    /// ファイルから読み込み
    pub fn load(path: &Path) -> io::Result<Self> {
        let tsv = fs::read_to_string(path)?;
        Ok(Self::from_tsv(&tsv, DEFAULT_CAPACITY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_get() {
        let mut s = LearningStore::new(10);
        s.record("", "きょう", "京");
        assert_eq!(s.get("", "きょう"), Some("京"));
        assert_eq!(s.get("", "あす"), None);
    }

    #[test]
    fn test_record_overwrites_same_yomi() {
        let mut s = LearningStore::new(10);
        s.record("", "きょう", "京");
        s.record("", "きょう", "今日");
        assert_eq!(s.get("", "きょう"), Some("今日"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_context_preferred_then_fallback() {
        let mut s = LearningStore::new(10);
        s.record("", "ふって", "振って");
        s.record("雨が", "ふって", "降って");
        assert_eq!(s.get("雨が", "ふって"), Some("降って"));
        assert_eq!(s.get("手を", "ふって"), Some("降って")); // 文脈不一致は最新を返す
        s.record("手を", "ふって", "振って");
        assert_eq!(s.get("手を", "ふって"), Some("振って"));
        assert_eq!(s.get("雨が", "ふって"), Some("降って"));
        assert_eq!(s.get("", "ふって"), Some("振って")); // 文脈なし = 最新
    }

    #[test]
    fn test_capacity_evicts_oldest() {
        let mut s = LearningStore::new(2);
        s.record("", "あ", "亜");
        s.record("", "い", "伊");
        s.record("", "う", "宇"); // 「あ」が追い出される
        assert_eq!(s.get("", "あ"), None);
        assert_eq!(s.get("", "い"), Some("伊"));
        assert_eq!(s.get("", "う"), Some("宇"));
    }

    #[test]
    fn test_record_refreshes_recency() {
        let mut s = LearningStore::new(2);
        s.record("", "あ", "亜");
        s.record("", "い", "伊");
        s.record("", "あ", "亜"); // 「あ」が最新になる
        s.record("", "う", "宇"); // 追い出されるのは「い」
        assert_eq!(s.get("", "あ"), Some("亜"));
        assert_eq!(s.get("", "い"), None);
    }

    #[test]
    fn test_tsv_roundtrip() {
        let mut s = LearningStore::new(10);
        s.record("", "きょう", "京");
        s.record("雨が", "ふって", "降って");
        let tsv = s.to_tsv();
        assert_eq!(tsv, "雨が\tふって\t降って\n\tきょう\t京\n");
        let restored = LearningStore::from_tsv(&tsv, 10);
        assert_eq!(restored.get("", "きょう"), Some("京"));
        assert_eq!(restored.get("雨が", "ふって"), Some("降って"));
        // recency 順が保たれる
        assert_eq!(restored.to_tsv(), tsv);
    }

    #[test]
    fn test_from_tsv_accepts_two_column_lines_and_skips_malformed() {
        let s = LearningStore::from_tsv("きょう\t京\nmalformed-line\n\t\nx\tあす\t明日\n", 10);
        assert_eq!(s.len(), 2);
        assert_eq!(s.get("", "きょう"), Some("京"));
        assert_eq!(s.get("x", "あす"), Some("明日"));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let mut s = LearningStore::new(10);
        s.record("", "きょう", "京");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("jamo-learning-test-{}.tsv", std::process::id()));
        s.save(&path).unwrap();
        let restored = LearningStore::load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(restored.get("", "きょう"), Some("京"));
    }
}
