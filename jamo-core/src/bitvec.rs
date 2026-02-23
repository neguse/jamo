//! 簡潔ビットベクトル (rank/select 対応)
//!
//! LOUDS trie の基盤データ構造。
//! 大ブロック (256bit) + 小ブロック (64bit) で O(1) rank を実現。

const SMALL_BLOCK_BITS: usize = 64;
const LARGE_BLOCK_SIZE: usize = 4; // 小ブロック4つ = 256bit

/// rank/select 対応のビットベクトル
pub struct BitVec {
    /// 生のビット列 (64bit 単位)
    blocks: Vec<u64>,
    /// ビット数
    len: usize,
    /// 大ブロックごとの累計 rank1 (大ブロック先頭までの1の個数)
    large_rank: Vec<u32>,
    /// 小ブロックごとの大ブロック内累計 (大ブロック先頭からの1の個数)
    small_rank: Vec<u8>,
}

impl BitVec {
    /// ビット列から構築
    pub fn from_bits(bits: &[bool]) -> Self {
        let len = bits.len();
        let num_blocks = len.div_ceil(SMALL_BLOCK_BITS);
        let mut blocks = vec![0u64; num_blocks];

        for (i, &b) in bits.iter().enumerate() {
            if b {
                blocks[i / SMALL_BLOCK_BITS] |= 1u64 << (i % SMALL_BLOCK_BITS);
            }
        }

        Self::from_raw(blocks, len)
    }

    /// u64 ブロック列とビット長から構築
    pub fn from_raw(blocks: Vec<u64>, len: usize) -> Self {
        let num_blocks = blocks.len();
        let num_large = num_blocks.div_ceil(LARGE_BLOCK_SIZE) + 1;

        let mut large_rank = vec![0u32; num_large];
        let mut small_rank = vec![0u8; num_blocks];

        let mut cumulative = 0u32;
        for (large_idx, lr) in large_rank.iter_mut().enumerate().take(num_large - 1) {
            *lr = cumulative;
            let base = large_idx * LARGE_BLOCK_SIZE;
            let mut within = 0u32;
            for s in 0..LARGE_BLOCK_SIZE {
                let block_idx = base + s;
                if block_idx < num_blocks {
                    small_rank[block_idx] = within as u8;
                    let popcnt = blocks[block_idx].count_ones();
                    within += popcnt;
                }
            }
            cumulative += within;
        }
        if num_large > 0 {
            large_rank[num_large - 1] = cumulative;
        }

        Self {
            blocks,
            len,
            large_rank,
            small_rank,
        }
    }

    /// ビット長
    pub fn len(&self) -> usize {
        self.len
    }

    /// 空か
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// i 番目のビットを取得
    pub fn get(&self, i: usize) -> bool {
        debug_assert!(i < self.len);
        let block = i / SMALL_BLOCK_BITS;
        let offset = i % SMALL_BLOCK_BITS;
        (self.blocks[block] >> offset) & 1 == 1
    }

    /// rank1(i): [0, i) の1の個数
    pub fn rank1(&self, i: usize) -> usize {
        debug_assert!(i <= self.len);
        if i == 0 {
            return 0;
        }
        let block_idx = (i - 1) / SMALL_BLOCK_BITS;
        let large_idx = block_idx / LARGE_BLOCK_SIZE;
        let offset = i % SMALL_BLOCK_BITS;

        let mut r = self.large_rank[large_idx] as usize + self.small_rank[block_idx] as usize;

        if offset == 0 {
            // i が 64 の倍数 → ブロック全体の popcount を加算
            r += self.blocks[block_idx].count_ones() as usize;
        } else {
            // ブロック内の [0, offset) ビットだけ数える
            let mask = (1u64 << offset) - 1;
            r += (self.blocks[block_idx] & mask).count_ones() as usize;
        }

        r
    }

    /// rank0(i): [0, i) の0の個数
    pub fn rank0(&self, i: usize) -> usize {
        i - self.rank1(i)
    }

    /// select1(k): k番目 (0-indexed) の1ビットの位置
    /// 見つからない場合は None
    pub fn select1(&self, k: usize) -> Option<usize> {
        let large_count = self.large_rank.len();
        if large_count == 0 {
            return None;
        }

        // 大ブロックを二分探索: large_rank[lo] <= k < large_rank[lo+1]
        let mut lo = 0usize;
        let mut hi = large_count - 1;
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if self.large_rank[mid] as usize <= k {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }

        let mut remaining = k - self.large_rank[lo] as usize;
        let base = lo * LARGE_BLOCK_SIZE;

        // 小ブロックを線形探索
        for s in 0..LARGE_BLOCK_SIZE {
            let block_idx = base + s;
            if block_idx >= self.blocks.len() {
                return None;
            }
            let popcnt = self.blocks[block_idx].count_ones() as usize;
            if remaining < popcnt {
                let pos = select_in_word(self.blocks[block_idx], remaining);
                let result = block_idx * SMALL_BLOCK_BITS + pos;
                return if result < self.len {
                    Some(result)
                } else {
                    None
                };
            }
            remaining -= popcnt;
        }

        None
    }

    /// select0(k): k番目 (0-indexed) の0ビットの位置
    pub fn select0(&self, k: usize) -> Option<usize> {
        let large_count = self.large_rank.len();
        if large_count == 0 {
            return None;
        }

        // 各大ブロック先頭までの0の個数
        let zero_count = |large_idx: usize| -> usize {
            large_idx * LARGE_BLOCK_SIZE * SMALL_BLOCK_BITS - self.large_rank[large_idx] as usize
        };

        let mut lo = 0usize;
        let mut hi = large_count - 1;
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if zero_count(mid) <= k {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }

        let mut remaining = k - zero_count(lo);
        let base = lo * LARGE_BLOCK_SIZE;

        for s in 0..LARGE_BLOCK_SIZE {
            let block_idx = base + s;
            if block_idx >= self.blocks.len() {
                return None;
            }
            let valid_bits = if block_idx == self.blocks.len() - 1
                && !self.len.is_multiple_of(SMALL_BLOCK_BITS)
            {
                self.len % SMALL_BLOCK_BITS
            } else if block_idx * SMALL_BLOCK_BITS < self.len {
                SMALL_BLOCK_BITS
            } else {
                return None;
            };
            let zero_cnt = valid_bits - self.blocks[block_idx].count_ones() as usize;
            if remaining < zero_cnt {
                let pos = select_in_word(!self.blocks[block_idx], remaining);
                let result = block_idx * SMALL_BLOCK_BITS + pos;
                return if result < self.len {
                    Some(result)
                } else {
                    None
                };
            }
            remaining -= zero_cnt;
        }

        None
    }
}

/// u64 内で k 番目 (0-indexed) の1ビットの位置
fn select_in_word(word: u64, k: usize) -> usize {
    let mut remaining = k;
    let mut w = word;
    for pos in 0..64 {
        if w & 1 == 1 {
            if remaining == 0 {
                return pos;
            }
            remaining -= 1;
        }
        w >>= 1;
    }
    unreachable!("select_in_word: not enough 1-bits")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_bits_and_get() {
        let bits = vec![true, false, true, true, false];
        let bv = BitVec::from_bits(&bits);
        assert_eq!(bv.len(), 5);
        assert!(bv.get(0));
        assert!(!bv.get(1));
        assert!(bv.get(2));
        assert!(bv.get(3));
        assert!(!bv.get(4));
    }

    #[test]
    fn test_rank1() {
        // bits: 1 0 1 1 0 1 0 0
        let bits = vec![true, false, true, true, false, true, false, false];
        let bv = BitVec::from_bits(&bits);

        assert_eq!(bv.rank1(0), 0);
        assert_eq!(bv.rank1(1), 1); // [0,1) = {1}
        assert_eq!(bv.rank1(2), 1); // [0,2) = {1,0}
        assert_eq!(bv.rank1(3), 2); // [0,3) = {1,0,1}
        assert_eq!(bv.rank1(4), 3); // [0,4) = {1,0,1,1}
        assert_eq!(bv.rank1(5), 3); // [0,5) = {1,0,1,1,0}
        assert_eq!(bv.rank1(6), 4); // [0,6) = {1,0,1,1,0,1}
        assert_eq!(bv.rank1(7), 4);
        assert_eq!(bv.rank1(8), 4);
    }

    #[test]
    fn test_rank0() {
        let bits = vec![true, false, true, true, false, true, false, false];
        let bv = BitVec::from_bits(&bits);

        assert_eq!(bv.rank0(0), 0);
        assert_eq!(bv.rank0(1), 0);
        assert_eq!(bv.rank0(2), 1);
        assert_eq!(bv.rank0(4), 1);
        assert_eq!(bv.rank0(5), 2);
        assert_eq!(bv.rank0(8), 4);
    }

    #[test]
    fn test_select1() {
        let bits = vec![true, false, true, true, false, true, false, false];
        let bv = BitVec::from_bits(&bits);

        assert_eq!(bv.select1(0), Some(0)); // 0番目の1 → 位置0
        assert_eq!(bv.select1(1), Some(2)); // 1番目の1 → 位置2
        assert_eq!(bv.select1(2), Some(3)); // 2番目の1 → 位置3
        assert_eq!(bv.select1(3), Some(5)); // 3番目の1 → 位置5
        assert_eq!(bv.select1(4), None); // 4番目の1は存在しない
    }

    #[test]
    fn test_select0() {
        let bits = vec![true, false, true, true, false, true, false, false];
        let bv = BitVec::from_bits(&bits);

        assert_eq!(bv.select0(0), Some(1)); // 0番目の0 → 位置1
        assert_eq!(bv.select0(1), Some(4)); // 1番目の0 → 位置4
        assert_eq!(bv.select0(2), Some(6)); // 2番目の0 → 位置6
        assert_eq!(bv.select0(3), Some(7)); // 3番目の0 → 位置7
        assert_eq!(bv.select0(4), None);
    }

    #[test]
    fn test_empty() {
        let bv = BitVec::from_bits(&[]);
        assert!(bv.is_empty());
        assert_eq!(bv.len(), 0);
        assert_eq!(bv.rank1(0), 0);
        assert_eq!(bv.rank0(0), 0);
        assert_eq!(bv.select1(0), None);
        assert_eq!(bv.select0(0), None);
    }

    #[test]
    fn test_all_ones() {
        let bits = vec![true; 10];
        let bv = BitVec::from_bits(&bits);
        assert_eq!(bv.rank1(10), 10);
        assert_eq!(bv.rank0(10), 0);
        assert_eq!(bv.select1(9), Some(9));
        assert_eq!(bv.select0(0), None);
    }

    #[test]
    fn test_all_zeros() {
        let bits = vec![false; 10];
        let bv = BitVec::from_bits(&bits);
        assert_eq!(bv.rank1(10), 0);
        assert_eq!(bv.rank0(10), 10);
        assert_eq!(bv.select1(0), None);
        assert_eq!(bv.select0(9), Some(9));
    }

    #[test]
    fn test_large_bitvec() {
        // 300ビット (複数の大ブロックにまたがる)
        let mut bits = vec![false; 300];
        // 位置 0, 64, 128, 192, 256, 299 に1を立てる
        let ones = [0, 64, 128, 192, 256, 299];
        for &pos in &ones {
            bits[pos] = true;
        }
        let bv = BitVec::from_bits(&bits);

        assert_eq!(bv.len(), 300);
        assert_eq!(bv.rank1(300), 6);
        assert_eq!(bv.rank0(300), 294);

        // select で各1の位置を確認
        for (k, &pos) in ones.iter().enumerate() {
            assert_eq!(bv.select1(k), Some(pos), "select1({k}) should be {pos}");
        }
        assert_eq!(bv.select1(6), None);
    }

    #[test]
    fn test_rank_select_consistency() {
        // rank1(select1(k)) == k+1 が成り立つか
        let bits = vec![
            false, true, false, true, true, false, false, true, true, true, false, true,
        ];
        let bv = BitVec::from_bits(&bits);

        let total_ones = bv.rank1(bv.len());
        for k in 0..total_ones {
            let pos = bv.select1(k).unwrap();
            assert_eq!(
                bv.rank1(pos + 1),
                k + 1,
                "rank1(select1({k}) + 1) should be {}",
                k + 1
            );
        }

        let total_zeros = bv.rank0(bv.len());
        for k in 0..total_zeros {
            let pos = bv.select0(k).unwrap();
            assert_eq!(
                bv.rank0(pos + 1),
                k + 1,
                "rank0(select0({k}) + 1) should be {}",
                k + 1
            );
        }
    }

    #[test]
    fn test_louds_pattern() {
        // LOUDS trie で実際に現れるパターンをテスト
        // ルート: 子2つ → "10 10 10 0 0 0"
        // 意味: ルートの子(2つ): 1,1,0  子1の子(1つ): 1,0  子2の子(0): 0
        let bits = vec![true, true, false, true, false, false];
        let bv = BitVec::from_bits(&bits);

        // rank0/rank1 が正しければ LOUDS のナビゲーションに使える
        assert_eq!(bv.rank1(3), 2); // ルートの子は2つ
        assert_eq!(bv.rank0(3), 1); // 最初の0の出現

        // select0(0) = 位置2 (最初のノードの終端)
        assert_eq!(bv.select0(0), Some(2));
    }
}
