//! 変換学習 (読み→確定表層の LRU)
//!
//! commit 時に「この読みをこの表層で確定した」を記録し、次回変換時に
//! その表層を文節候補の先頭に持ってくる。容量上限つきで古いものから消える。
//! 永続化はホスト提供のパスへの TSV (recency 順、先頭が最新)。

use std::fs;
use std::io;
use std::path::Path;

const DEFAULT_CAPACITY: usize = 1024;

/// 読み→表層の学習ストア (recency 順、先頭が最新)
#[derive(Debug, Clone)]
pub struct LearningStore {
    /// (読み, 表層)。先頭が最新
    entries: Vec<(String, String)>,
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

    /// 確定を記録する。同じ読みの既存エントリは上書きされ、最新になる
    pub fn record(&mut self, yomi: &str, surface: &str) {
        if yomi.is_empty() || surface.is_empty() {
            return;
        }
        self.entries.retain(|(y, _)| y != yomi);
        self.entries
            .insert(0, (yomi.to_string(), surface.to_string()));
        self.entries.truncate(self.capacity);
    }

    /// 読みに対する学習済み表層
    pub fn get(&self, yomi: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(y, _)| y == yomi)
            .map(|(_, s)| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// TSV 形式にシリアライズ (recency 順)
    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        for (yomi, surface) in &self.entries {
            out.push_str(yomi);
            out.push('\t');
            out.push_str(surface);
            out.push('\n');
        }
        out
    }

    /// TSV から復元。不正な行は読み飛ばす
    pub fn from_tsv(tsv: &str, capacity: usize) -> Self {
        let mut store = Self::new(capacity);
        for line in tsv.lines() {
            let mut it = line.splitn(2, '\t');
            if let (Some(yomi), Some(surface)) = (it.next(), it.next())
                && !yomi.is_empty()
                && !surface.is_empty()
                && store.entries.len() < store.capacity
            {
                store.entries.push((yomi.to_string(), surface.to_string()));
            }
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
        s.record("きょう", "京");
        assert_eq!(s.get("きょう"), Some("京"));
        assert_eq!(s.get("あす"), None);
    }

    #[test]
    fn test_record_overwrites_same_yomi() {
        let mut s = LearningStore::new(10);
        s.record("きょう", "京");
        s.record("きょう", "今日");
        assert_eq!(s.get("きょう"), Some("今日"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_capacity_evicts_oldest() {
        let mut s = LearningStore::new(2);
        s.record("あ", "亜");
        s.record("い", "伊");
        s.record("う", "宇"); // 「あ」が追い出される
        assert_eq!(s.get("あ"), None);
        assert_eq!(s.get("い"), Some("伊"));
        assert_eq!(s.get("う"), Some("宇"));
    }

    #[test]
    fn test_record_refreshes_recency() {
        let mut s = LearningStore::new(2);
        s.record("あ", "亜");
        s.record("い", "伊");
        s.record("あ", "亜"); // 「あ」が最新になる
        s.record("う", "宇"); // 追い出されるのは「い」
        assert_eq!(s.get("あ"), Some("亜"));
        assert_eq!(s.get("い"), None);
    }

    #[test]
    fn test_tsv_roundtrip() {
        let mut s = LearningStore::new(10);
        s.record("きょう", "京");
        s.record("あす", "明日");
        let tsv = s.to_tsv();
        let restored = LearningStore::from_tsv(&tsv, 10);
        assert_eq!(restored.get("きょう"), Some("京"));
        assert_eq!(restored.get("あす"), Some("明日"));
        // recency 順が保たれる (先頭 = 最新 = あす)
        assert_eq!(restored.to_tsv(), tsv);
    }

    #[test]
    fn test_from_tsv_skips_malformed() {
        let s = LearningStore::from_tsv("きょう\t京\nmalformed-line\n\t\nあす\t明日\n", 10);
        assert_eq!(s.len(), 2);
        assert_eq!(s.get("きょう"), Some("京"));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let mut s = LearningStore::new(10);
        s.record("きょう", "京");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("jamo-learning-test-{}.tsv", std::process::id()));
        s.save(&path).unwrap();
        let restored = LearningStore::load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(restored.get("きょう"), Some("京"));
    }
}
