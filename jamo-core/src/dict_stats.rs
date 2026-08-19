//! 辞書バイナリのセクション別サイズ内訳 (`jamo dict info`)。
//!
//! doc/dict-format.md のレイアウトに準拠してバイトスライスを走査する。
//! ロード (ipadic::read_binary) とは独立で、構造の妥当性検証はしない。
//! サイズ削減の計測用途 (doc/memory.md)。

use std::collections::HashSet;

pub struct DictStats {
    pub version: u32,
    pub total_bytes: usize,
    pub header_bytes: usize,
    pub louds_bytes: usize,
    pub louds_bits: u32,
    pub labels_bytes: usize,
    pub node_count: u32,
    pub terminal_bytes: usize,
    pub terminal_count: u32,
    /// 終端でないノード数 (terminal のセンチネル 0xFFFFFFFF)
    pub sentinel_count: u32,
    pub entries_bytes: usize,
    pub entry_count: u32,
    /// 表層形の合計バイト数 (長さプレフィクス除く)
    pub surface_bytes: usize,
    pub surface_uniq_count: usize,
    pub surface_uniq_bytes: usize,
    /// エントリ固定部 (surface_len + left/right/cost/pos_type) の合計
    pub entry_fixed_bytes: usize,
    pub matrix_bytes: usize,
    pub num_pos: u32,
    pub stats_bytes: usize,
    pub vocab_size: u32,
    pub reading_count: u32,
}

struct Reader<'a> {
    data: &'a [u8],
    off: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .off
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or_else(|| format!("truncated at byte {}", self.off))?;
        let slice = &self.data[self.off..end];
        self.off = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// 辞書バイナリを走査してセクション別サイズを返す
pub fn dict_stats(data: &[u8]) -> Result<DictStats, String> {
    let mut r = Reader { data, off: 0 };
    if r.take(4)? != b"JAMO" {
        return Err("bad magic (not a jamo dict)".into());
    }
    let version = r.u32()?;
    if version != 2 {
        return Err(format!("unsupported version {version}"));
    }
    let header_bytes = r.off;

    let louds_start = r.off;
    let louds_bits = r.u32()?;
    let block_count = r.u32()? as usize;
    let block_bytes = block_count
        .checked_mul(8)
        .ok_or("louds block count overflow")?;
    r.take(block_bytes)?;
    let louds_bytes = r.off - louds_start;

    let labels_start = r.off;
    let node_count = r.u32()?;
    r.take(node_count as usize)?;
    let labels_bytes = r.off - labels_start;

    let terminal_start = r.off;
    let terminal_count = r.u32()?;
    let mut sentinel_count = 0u32;
    for _ in 0..terminal_count {
        let start = r.u32()?;
        let _end = r.u32()?;
        if start == u32::MAX {
            sentinel_count += 1;
        }
    }
    let terminal_bytes = r.off - terminal_start;

    let entries_start = r.off;
    let entry_count = r.u32()?;
    let mut surface_bytes = 0usize;
    let mut uniq: HashSet<&[u8]> = HashSet::new();
    for _ in 0..entry_count {
        let len = r.u16()? as usize;
        let surface = r.take(len)?;
        uniq.insert(surface);
        surface_bytes += len;
        r.take(7)?; // left_id, right_id, cost, pos_type
    }
    let entries_bytes = r.off - entries_start;
    let surface_uniq_bytes = uniq.iter().map(|s| s.len()).sum();
    let surface_uniq_count = uniq.len();

    let matrix_start = r.off;
    let num_pos = r.u32()?;
    let matrix_data_bytes = (num_pos as usize)
        .checked_mul(num_pos as usize)
        .and_then(|v| v.checked_mul(2))
        .ok_or("matrix size overflow")?;
    r.take(matrix_data_bytes)?;
    let matrix_bytes = r.off - matrix_start;

    let stats_start = r.off;
    let vocab_size = r.u32()?;
    let reading_count = r.u32()?;
    let stats_bytes = r.off - stats_start;

    if r.off != data.len() {
        return Err(format!("trailing {} bytes", data.len() - r.off));
    }

    Ok(DictStats {
        version,
        total_bytes: data.len(),
        header_bytes,
        louds_bytes,
        louds_bits,
        labels_bytes,
        node_count,
        terminal_bytes,
        terminal_count,
        sentinel_count,
        entries_bytes,
        entry_count,
        surface_bytes,
        surface_uniq_count,
        surface_uniq_bytes,
        entry_fixed_bytes: entries_bytes - 4 - surface_bytes,
        matrix_bytes,
        num_pos,
        stats_bytes,
        vocab_size,
        reading_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipadic::IpaDict;

    /// IPAdic 非依存のミニ辞書を一時ディレクトリに作ってロードする
    fn build_mini_dict(tag: &str) -> (std::path::PathBuf, IpaDict) {
        let tmp_dir = std::env::temp_dir().join(format!("jamo_dict_stats_{tag}"));
        let _ = std::fs::create_dir_all(&tmp_dir);
        std::fs::write(
            tmp_dir.join("mini.csv"),
            "今日,0,0,3000,名詞,一般,*,*,*,*,今日,キョウ,キョウ\n\
             教,1,1,4000,名詞,一般,*,*,*,*,教,キョウ,キョウ\n\
             は,1,0,500,助詞,係助詞,*,*,*,*,は,ハ,ワ\n",
        )
        .unwrap();
        std::fs::write(
            tmp_dir.join("matrix.def"),
            "2 2\n0 0 0\n0 1 10\n1 0 20\n1 1 30\n",
        )
        .unwrap();
        let dict = IpaDict::load(&tmp_dir).unwrap();
        (tmp_dir, dict)
    }

    #[test]
    fn test_dict_stats_sections_add_up() {
        let (tmp_dir, dict) = build_mini_dict("sections");
        let bin = tmp_dir.join("mini.dict");
        dict.save_binary(&bin).unwrap();
        let bytes = std::fs::read(&bin).unwrap();

        let s = dict_stats(&bytes).unwrap();
        assert_eq!(s.version, 2);
        assert_eq!(s.total_bytes, bytes.len());
        assert_eq!(
            s.header_bytes
                + s.louds_bytes
                + s.labels_bytes
                + s.terminal_bytes
                + s.entries_bytes
                + s.matrix_bytes
                + s.stats_bytes,
            s.total_bytes
        );
        assert_eq!(s.entry_count, 3);
        assert_eq!(s.num_pos, 2);
        assert_eq!(s.vocab_size, 3);
        assert_eq!(s.reading_count, 2); // きょう, は
        assert_eq!(s.node_count, s.terminal_count);
        assert!(s.sentinel_count < s.terminal_count);
        // 表層: 今日(6) + 教(3) + は(3)、全てユニーク
        assert_eq!(s.surface_bytes, 12);
        assert_eq!(s.surface_uniq_count, 3);
        assert_eq!(s.surface_uniq_bytes, 12);
        // 固定部 9B/エントリ
        assert_eq!(s.entry_fixed_bytes, 9 * 3);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_dict_stats_bad_magic() {
        assert!(dict_stats(b"XXXX\x02\x00\x00\x00").is_err());
    }

    #[test]
    fn test_dict_stats_truncated() {
        let (tmp_dir, dict) = build_mini_dict("truncated");
        let bin = tmp_dir.join("mini.dict");
        dict.save_binary(&bin).unwrap();
        let bytes = std::fs::read(&bin).unwrap();

        for cut in [0, 4, 8, 20, bytes.len() - 1] {
            assert!(
                dict_stats(&bytes[..cut]).is_err(),
                "cut at {cut} should fail"
            );
        }
        // 末尾にゴミを足してもエラー
        let mut padded = bytes.clone();
        padded.push(0);
        assert!(dict_stats(&padded).is_err());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
