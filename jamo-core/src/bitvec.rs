//! rank/select 付きビットベクトル
//!
//! `BitVec` はビルド時にビット列を組み立てて直列化する所有型、
//! `BitView` は直列化されたバイト列を借用して rank/select を引くゼロコピー型。
//! rank 索引と select サンプルもファイルに含めるので、ロード時の再構築は不要。
//!
//! 直列化レイアウト (全てリトルエンディアン):
//!
//! ```text
//! u32 len            ビット数
//! u32 nblocks        64bit ブロック数 (= ceil(len / 64))
//! u32 sample         select サンプル間隔 (0 = サンプルなし、select は使えない)
//! u32 reserved
//! u64 × nblocks      ビット本体
//! u64 × ndir         rank 索引 (大ブロック 256bit ごと、ndir = ceil(nblocks/4) + 1):
//!                    bit 0..32 = 大ブロック先頭までの 1 の累積数、
//!                    bit 32..40 / 40..48 / 48..56 = 2〜4 番目の小ブロック先頭までの 1 の数
//!                    末尾は番兵 (累積 = 1 の総数)
//! u32 × nsel1        select1 サンプル: sample 個ごとの 1 の位置 (nsel1 = ceil(ones / sample))
//! u32 × nsel0        select0 サンプル: sample 個ごとの 0 の位置 (nsel0 = ceil(zeros / sample))
//! ```

use std::io;
use std::ops::Range;

const BLOCK_BITS: usize = 64;
const LARGE_BLOCKS: usize = 4; // 小ブロック4つ = 256bit
const SELECT_SAMPLE: usize = 64;

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// ビルド用の所有ビット列
#[derive(Debug, Clone, Default)]
pub struct BitVec {
    blocks: Vec<u64>,
    len: usize,
}

impl BitVec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_bits(bits: &[bool]) -> Self {
        let mut v = Self::new();
        for &b in bits {
            v.push(b);
        }
        v
    }

    pub fn push(&mut self, bit: bool) {
        if self.len.is_multiple_of(BLOCK_BITS) {
            self.blocks.push(0);
        }
        if bit {
            self.blocks[self.len / BLOCK_BITS] |= 1u64 << (self.len % BLOCK_BITS);
        }
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// rank 索引・select サンプル込みで `out` に直列化する
    pub fn append_to(&self, out: &mut Vec<u8>) {
        self.append_to_with(out, true);
    }

    /// rank 索引のみ (select サンプルなし) で直列化する。select は使えない
    pub fn append_rank_only(&self, out: &mut Vec<u8>) {
        self.append_to_with(out, false);
    }

    fn append_to_with(&self, out: &mut Vec<u8>, with_select: bool) {
        let nblocks = self.blocks.len();
        out.extend_from_slice(&(self.len as u32).to_le_bytes());
        out.extend_from_slice(&(nblocks as u32).to_le_bytes());
        let sample = if with_select { SELECT_SAMPLE } else { 0 };
        out.extend_from_slice(&(sample as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for &b in &self.blocks {
            out.extend_from_slice(&b.to_le_bytes());
        }
        let ndir = nblocks.div_ceil(LARGE_BLOCKS) + 1;
        let mut cumulative = 0u64;
        for d in 0..ndir {
            let mut entry = cumulative;
            let mut within = 0u64;
            for s in 0..LARGE_BLOCKS {
                if s > 0 {
                    entry |= within << (24 + 8 * s);
                }
                if let Some(&b) = self.blocks.get(d * LARGE_BLOCKS + s) {
                    within += b.count_ones() as u64;
                }
            }
            out.extend_from_slice(&entry.to_le_bytes());
            cumulative += within;
        }
        if !with_select {
            return;
        }
        let mut ones = 0usize;
        let mut zeros = 0usize;
        let mut sel1 = Vec::new();
        let mut sel0 = Vec::new();
        for i in 0..self.len {
            if (self.blocks[i / BLOCK_BITS] >> (i % BLOCK_BITS)) & 1 == 1 {
                if ones.is_multiple_of(SELECT_SAMPLE) {
                    sel1.push(i as u32);
                }
                ones += 1;
            } else {
                if zeros.is_multiple_of(SELECT_SAMPLE) {
                    sel0.push(i as u32);
                }
                zeros += 1;
            }
        }
        for p in sel1.iter().chain(sel0.iter()) {
            out.extend_from_slice(&p.to_le_bytes());
        }
    }
}

/// 直列化済みビット列のファイル内配置 (`BitView` を安く作り直すための記録)
#[derive(Debug, Clone)]
pub struct BitRef {
    blocks: Range<usize>,
    dir: Range<usize>,
    sel1: Range<usize>,
    sel0: Range<usize>,
    len: usize,
}

impl BitRef {
    /// `data` 先頭から 1 本分の配置を読み、(配置, 消費バイト数) を返す。
    /// 配置は `data` の先頭を 0 とする相対オフセット
    pub fn parse(data: &[u8]) -> io::Result<(Self, usize)> {
        if data.len() < 16 {
            return Err(invalid("bitvec header truncated"));
        }
        let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let nblocks = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let sample = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        if nblocks != len.div_ceil(BLOCK_BITS) {
            return Err(invalid(format!(
                "bitvec block count mismatch: len={len}, nblocks={nblocks}"
            )));
        }
        if sample != 0 && sample != SELECT_SAMPLE {
            return Err(invalid(format!(
                "bitvec unsupported select sample {sample}"
            )));
        }
        let ndir = nblocks.div_ceil(LARGE_BLOCKS) + 1;
        let blocks = 16..16 + nblocks * 8;
        let dir = blocks.end..blocks.end + ndir * 8;
        if data.len() < dir.end {
            return Err(invalid("bitvec data truncated"));
        }
        let s = &data[dir.end - 8..dir.end];
        let ones = u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize;
        if ones > len {
            return Err(invalid("bitvec ones count exceeds len"));
        }
        let (nsel1, nsel0) = if sample == 0 {
            (0, 0)
        } else {
            (ones.div_ceil(sample), (len - ones).div_ceil(sample))
        };
        let sel1 = dir.end..dir.end + nsel1 * 4;
        let sel0 = sel1.end..sel1.end + nsel0 * 4;
        if data.len() < sel0.end {
            return Err(invalid("bitvec select samples truncated"));
        }
        let used = sel0.end;
        Ok((
            Self {
                blocks,
                dir,
                sel1,
                sel0,
                len,
            },
            used,
        ))
    }

    /// 相対オフセットを `base` だけずらす
    pub fn offset(mut self, base: usize) -> Self {
        for r in [
            &mut self.blocks,
            &mut self.dir,
            &mut self.sel1,
            &mut self.sel0,
        ] {
            r.start += base;
            r.end += base;
        }
        self
    }
}

/// 直列化済みビット列の借用ビュー
#[derive(Clone, Copy)]
pub struct BitView<'a> {
    blocks: &'a [u8],
    dir: &'a [u8],
    sel1: &'a [u8],
    sel0: &'a [u8],
    len: usize,
}

impl<'a> BitView<'a> {
    /// `data` 先頭から 1 本パースし、(ビュー, 消費バイト数) を返す
    pub fn parse(data: &'a [u8]) -> io::Result<(Self, usize)> {
        let (r, used) = BitRef::parse(data)?;
        Ok((Self::from_ref(data, &r), used))
    }

    /// 記録済みの配置からビューを作る (検証なし)
    #[inline]
    pub fn from_ref(data: &'a [u8], r: &BitRef) -> Self {
        Self {
            blocks: &data[r.blocks.clone()],
            dir: &data[r.dir.clone()],
            sel1: &data[r.sel1.clone()],
            sel0: &data[r.sel0.clone()],
            len: r.len,
        }
    }

    /// 索引・サンプルが本体と整合しているか検証する (ロード時の深い検証用)
    pub fn verify(&self) -> io::Result<()> {
        let nblocks = self.blocks.len() / 8;
        let mut cumulative = 0u64;
        for d in 0..self.dir.len() / 8 {
            let mut expect = cumulative;
            let mut within = 0u64;
            for s in 0..LARGE_BLOCKS {
                if s > 0 {
                    expect |= within << (24 + 8 * s);
                }
                let i = d * LARGE_BLOCKS + s;
                if i < nblocks {
                    within += self.block(i).count_ones() as u64;
                }
            }
            if self.dir_entry(d) != expect {
                return Err(invalid(format!("bitvec rank directory mismatch at {d}")));
            }
            cumulative += within;
        }
        if !self.len.is_multiple_of(BLOCK_BITS)
            && nblocks > 0
            && self.block(nblocks - 1) >> (self.len % BLOCK_BITS) != 0
        {
            return Err(invalid("bitvec has garbage past len"));
        }
        if !self.has_select() {
            return Ok(());
        }
        let (mut ones, mut zeros) = (0usize, 0usize);
        for i in 0..self.len {
            if self.get(i) {
                if ones.is_multiple_of(SELECT_SAMPLE) && self.sample(self.sel1, ones) != i {
                    return Err(invalid(format!("bitvec select1 sample mismatch at {i}")));
                }
                ones += 1;
            } else {
                if zeros.is_multiple_of(SELECT_SAMPLE) && self.sample(self.sel0, zeros) != i {
                    return Err(invalid(format!("bitvec select0 sample mismatch at {i}")));
                }
                zeros += 1;
            }
        }
        Ok(())
    }

    #[inline]
    fn block(&self, i: usize) -> u64 {
        read_u64(self.blocks, i * 8)
    }

    #[inline]
    fn dir_entry(&self, i: usize) -> u64 {
        read_u64(self.dir, i * 8)
    }

    /// select サンプルを持つか (持たなければ select1/select0 は常に None)
    pub fn has_select(&self) -> bool {
        !self.sel1.is_empty() || !self.sel0.is_empty() || self.len == 0
    }

    /// サンプル表の k 番目に対応する位置 (k は SELECT_SAMPLE の倍数)
    #[inline]
    fn sample(&self, table: &[u8], k: usize) -> usize {
        let i = (k / SELECT_SAMPLE) * 4;
        match table.get(i..i + 4) {
            Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize,
            None => self.len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 1 の総数 (索引の番兵から O(1))
    #[inline]
    pub fn count_ones(&self) -> usize {
        let d = self.dir.len() / 8;
        if d == 0 {
            0
        } else {
            (self.dir_entry(d - 1) & 0xFFFF_FFFF) as usize
        }
    }

    #[inline]
    pub fn get(&self, i: usize) -> bool {
        debug_assert!(i < self.len);
        (self.block(i / BLOCK_BITS) >> (i % BLOCK_BITS)) & 1 == 1
    }

    /// [0, i) の 1 の数
    #[inline]
    pub fn rank1(&self, i: usize) -> usize {
        debug_assert!(i <= self.len);
        if i == 0 {
            return 0;
        }
        let block_idx = (i - 1) / BLOCK_BITS;
        let offset = i % BLOCK_BITS;
        let word = self.block(block_idx);
        let before = if offset == 0 {
            word.count_ones() as usize
        } else {
            (word & ((1u64 << offset) - 1)).count_ones() as usize
        };
        self.rank_at_block(block_idx) + before
    }

    /// ブロック先頭までの 1 の数 (索引から)
    #[inline]
    fn rank_at_block(&self, block_idx: usize) -> usize {
        let entry = self.dir_entry(block_idx / LARGE_BLOCKS);
        let sub = block_idx % LARGE_BLOCKS;
        let mut r = (entry & 0xFFFF_FFFF) as usize;
        if sub > 0 {
            r += ((entry >> (24 + 8 * sub)) & 0xFF) as usize;
        }
        r
    }

    /// ビット i が立っていれば Some(rank1(i)) (1 回のブロック読みで判定と rank を行う)
    #[inline]
    pub fn rank1_if_set(&self, i: usize) -> Option<usize> {
        debug_assert!(i < self.len);
        let block_idx = i / BLOCK_BITS;
        let offset = i % BLOCK_BITS;
        let word = self.block(block_idx);
        if (word >> offset) & 1 == 0 {
            return None;
        }
        let before = (word & ((1u64 << offset) - 1)).count_ones() as usize;
        Some(self.rank_at_block(block_idx) + before)
    }

    /// [0, i) の 0 の数
    #[inline]
    pub fn rank0(&self, i: usize) -> usize {
        i - self.rank1(i)
    }

    /// `i` から始まる連続した 1 の数 (LOUDS の子の数)
    pub fn run_of_ones(&self, i: usize) -> usize {
        let mut count = 0;
        let mut pos = i;
        while pos < self.len {
            let word = self.block(pos / BLOCK_BITS) >> (pos % BLOCK_BITS);
            let valid = (self.len - pos).min(BLOCK_BITS - pos % BLOCK_BITS);
            let ones = (!word).trailing_zeros() as usize;
            if ones < valid {
                return count + ones;
            }
            count += valid;
            pos += valid;
        }
        count
    }

    /// k 番目 (0 始まり) の 1 の位置
    pub fn select1(&self, k: usize) -> Option<usize> {
        if k >= self.count_ones() {
            return None;
        }
        let start = self.sample(self.sel1, k);
        let mut block_idx = (start / BLOCK_BITS).min(self.blocks.len() / 8);
        // サンプルがない / 壊れている場合は None (panic しない)
        let mut remaining = k.checked_sub(self.rank1(block_idx * BLOCK_BITS))?;
        let nblocks = self.blocks.len() / 8;
        while block_idx < nblocks {
            let word = self.block(block_idx);
            let popcnt = word.count_ones() as usize;
            if remaining < popcnt {
                return Some(block_idx * BLOCK_BITS + select_in_word(word, remaining));
            }
            remaining -= popcnt;
            block_idx += 1;
        }
        None
    }

    /// k 番目 (0 始まり) の 0 の位置
    pub fn select0(&self, k: usize) -> Option<usize> {
        if k >= self.len - self.count_ones() {
            return None;
        }
        let start = self.sample(self.sel0, k);
        let mut block_idx = (start / BLOCK_BITS).min(self.blocks.len() / 8);
        let mut remaining = k.checked_sub(self.rank0(block_idx * BLOCK_BITS))?;
        let nblocks = self.blocks.len() / 8;
        while block_idx < nblocks {
            let valid = (self.len - block_idx * BLOCK_BITS).min(BLOCK_BITS);
            let word = self.block(block_idx);
            let zeros = valid - word.count_ones() as usize;
            if remaining < zeros {
                return Some(block_idx * BLOCK_BITS + select_in_word(!word, remaining));
            }
            remaining -= zeros;
            block_idx += 1;
        }
        None
    }
}

#[inline]
fn read_u64(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap_or([0; 8]))
}

/// word 内の k 番目 (0 始まり) の 1 の位置 (存在すること)
#[inline]
fn select_in_word(word: u64, k: usize) -> usize {
    let mut w = word;
    for _ in 0..k {
        w &= w - 1;
    }
    w.trailing_zeros() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_of(bits: &[bool]) -> Vec<u8> {
        let mut out = Vec::new();
        BitVec::from_bits(bits).append_to(&mut out);
        out
    }

    fn pseudo_random_bits(n: usize, modulo: u32) -> Vec<bool> {
        let mut state = 12345u32;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(1103515245).wrapping_add(12345);
                ((state >> 16) % modulo) == 0
            })
            .collect()
    }

    fn check_all(bits: &[bool]) {
        let bytes = view_of(bits);
        let (bv, used) = BitView::parse(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        bv.verify().unwrap();
        assert_eq!(bv.len(), bits.len());
        let mut ones = 0;
        for (i, &b) in bits.iter().enumerate() {
            assert_eq!(bv.get(i), b, "get({i})");
            assert_eq!(bv.rank1(i), ones, "rank1({i})");
            assert_eq!(bv.rank0(i), i - ones, "rank0({i})");
            if b {
                assert_eq!(bv.select1(ones), Some(i), "select1({ones})");
                ones += 1;
            } else {
                assert_eq!(bv.select0(i - ones), Some(i), "select0({})", i - ones);
            }
            let run = bits[i..].iter().take_while(|&&x| x).count();
            assert_eq!(bv.run_of_ones(i), run, "run_of_ones({i})");
        }
        assert_eq!(bv.rank1(bits.len()), ones);
        assert_eq!(bv.count_ones(), ones);
        assert_eq!(bv.select1(ones), None);
        assert_eq!(bv.select0(bits.len() - ones), None);
    }

    #[test]
    fn test_small_patterns() {
        check_all(&[true, false, true, true, false, true, false, false]);
        check_all(&[true, true, false, true, false, false]); // LOUDS
        check_all(&[true; 10]);
        check_all(&[false; 10]);
        check_all(&[true; 64]);
        check_all(&[false; 65]);
    }

    #[test]
    fn test_empty() {
        let bytes = view_of(&[]);
        let (bv, _) = BitView::parse(&bytes).unwrap();
        bv.verify().unwrap();
        assert!(bv.is_empty());
        assert_eq!(bv.rank1(0), 0);
        assert_eq!(bv.count_ones(), 0);
        assert_eq!(bv.select1(0), None);
        assert_eq!(bv.select0(0), None);
        assert_eq!(bv.run_of_ones(0), 0);
    }

    #[test]
    fn test_large_random_dense_and_sparse() {
        // select サンプル (64 個ごと) と rank 索引 (256bit ごと) をまたぐ長さ
        check_all(&pseudo_random_bits(3000, 2));
        check_all(&pseudo_random_bits(3000, 7));
        check_all(&pseudo_random_bits(3000, 97)); // 疎: サンプルから複数ブロック走査
        let mut mostly_ones = vec![true; 3000];
        for i in (0..3000).step_by(113) {
            mostly_ones[i] = false;
        }
        check_all(&mostly_ones);
    }

    #[test]
    fn test_run_of_ones_across_blocks() {
        let mut bits = vec![false; 10];
        bits.extend(vec![true; 130]);
        bits.push(false);
        let bytes = view_of(&bits);
        let (bv, _) = BitView::parse(&bytes).unwrap();
        assert_eq!(bv.run_of_ones(10), 130);
        assert_eq!(bv.run_of_ones(63), 77);
    }

    #[test]
    fn test_parse_rejects_truncated_and_mismatch() {
        let bytes = view_of(&[true; 100]);
        assert!(BitView::parse(&bytes[..bytes.len() - 1]).is_err());
        assert!(BitView::parse(&bytes[..4]).is_err());
        assert!(BitView::parse(&bytes[..24]).is_err());
        let mut bad = bytes.clone();
        bad[4] = 9; // nblocks を壊す
        assert!(BitView::parse(&bad).is_err());
        // 番兵の 1 の数が len を超える
        let mut bad = bytes.clone();
        let dir_end = 16 + 2 * 8 + 2 * 8;
        bad[dir_end - 8] = 200;
        assert!(BitView::parse(&bad).is_err());
        // 未知のサンプル間隔
        let mut bad = bytes.clone();
        bad[8] = 7;
        assert!(BitView::parse(&bad).is_err());
    }

    #[test]
    fn test_verify_detects_corruption() {
        let bits = pseudo_random_bits(500, 3);
        let bytes = view_of(&bits);
        let (bv, _) = BitView::parse(&bytes).unwrap();
        bv.verify().unwrap();
        let nblocks = 500usize.div_ceil(64);
        let dir_off = 16 + nblocks * 8;
        let sel_off = dir_off + (nblocks.div_ceil(4) + 1) * 8;

        let mut bad = bytes.clone();
        bad[dir_off + 8 + 5] ^= 1; // 2 番目の索引の小ブロック値
        assert!(BitView::parse(&bad).unwrap().0.verify().is_err());

        let mut bad = bytes.clone();
        bad[sel_off] ^= 1; // select1 サンプル
        assert!(BitView::parse(&bad).unwrap().0.verify().is_err());

        let mut garbage = bytes.clone();
        garbage[16 + nblocks * 8 - 1] |= 0x80; // len=500 なのに bit 511 が立っている
        assert!(BitView::parse(&garbage).unwrap().0.verify().is_err());
    }

    #[test]
    fn test_rank_only_serialization() {
        let bits = pseudo_random_bits(1000, 3);
        let mut out = Vec::new();
        BitVec::from_bits(&bits).append_rank_only(&mut out);
        let (bv, used) = BitView::parse(&out).unwrap();
        assert_eq!(used, out.len());
        bv.verify().unwrap();
        assert!(!bv.has_select());
        let ones = bits.iter().filter(|&&b| b).count();
        assert_eq!(bv.count_ones(), ones);
        for (i, &b) in bits.iter().enumerate() {
            let expect = if b { Some(bv.rank1(i)) } else { None };
            assert_eq!(bv.rank1_if_set(i), expect, "rank1_if_set({i})");
        }
        assert_eq!(bv.select1(0), None);
        assert_eq!(bv.select0(0), None);
        let mut full = Vec::new();
        BitVec::from_bits(&bits).append_to(&mut full);
        assert_eq!(
            full.len(),
            out.len() + (ones.div_ceil(64) + (1000 - ones).div_ceil(64)) * 4
        );
    }

    #[test]
    fn test_bitref_offset_roundtrip() {
        let bits = pseudo_random_bits(300, 2);
        let mut file = vec![0xAA; 13];
        BitVec::from_bits(&bits).append_to(&mut file);
        let (r, used) = BitRef::parse(&file[13..]).unwrap();
        assert_eq!(used, file.len() - 13);
        let bv = BitView::from_ref(&file, &r.offset(13));
        bv.verify().unwrap();
        assert_eq!(bv.count_ones(), bits.iter().filter(|&&b| b).count());
    }
}
