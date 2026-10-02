//! 読み・表層の文字符号化 (v3 辞書)
//!
//! - 読み: 出現頻度上位 255 文字を 1 バイト (1..=255) にする。表にない文字は
//!   エスケープ 0 + UTF-8 バイト列。mozc の読みは 255 文字種でぎりぎり収まる
//! - 表層: 上位 224 文字を 1 バイト (0..224)、次の 7936 文字を 2 バイト
//!   (prefix 224..255 + 1 バイト)、残りを 3 バイト (prefix 255 + 2 バイト)
//!
//! どちらの表も「u32 count + u32 コードポイント × count」で直列化する。

use std::collections::HashMap;

/// 読み符号の最大文字数 (1 バイト符号 1..=255)
pub const READING_TABLE_MAX: usize = 255;
/// 表層の 1 バイト符号数
const SURFACE_SINGLE: usize = 224;
/// 表層の 2 バイト prefix 数 (224..255)
const SURFACE_PREFIX2: usize = 31;
/// 表層の 2 バイト符号数
const SURFACE_DOUBLE: usize = SURFACE_PREFIX2 * 256;
/// 表層の 3 バイト符号数 (prefix 255)
const SURFACE_TRIPLE: usize = 65536;
/// 表層表の最大文字数
pub const SURFACE_TABLE_MAX: usize = SURFACE_SINGLE + SURFACE_DOUBLE + SURFACE_TRIPLE;

/// 文字表を直列化する (u32 count + u32 × count)
pub fn write_table(chars: &[char], out: &mut Vec<u8>) {
    out.extend_from_slice(&(chars.len() as u32).to_le_bytes());
    for &c in chars {
        out.extend_from_slice(&(c as u32).to_le_bytes());
    }
}

/// 直列化した文字表を読む。不正なコードポイントはエラー
pub fn read_table(data: &[u8]) -> Result<Vec<char>, String> {
    if data.len() < 4 {
        return Err("char table truncated".into());
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + count * 4 {
        return Err("char table truncated".into());
    }
    let mut chars = Vec::with_capacity(count);
    for i in 0..count {
        let b = &data[4 + i * 4..8 + i * 4];
        let cp = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        chars.push(char::from_u32(cp).ok_or_else(|| format!("bad code point {cp:#x}"))?);
    }
    Ok(chars)
}

/// 頻度順 (同率は文字順) に並べた文字表を作る
pub fn frequency_table<'a>(texts: impl Iterator<Item = &'a str>, limit: usize) -> Vec<char> {
    let mut freq: HashMap<char, u64> = HashMap::new();
    for t in texts {
        for c in t.chars() {
            *freq.entry(c).or_default() += 1;
        }
    }
    let mut v: Vec<(char, u64)> = freq.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(limit);
    v.into_iter().map(|(c, _)| c).collect()
}

/// 読みの符号化表 (文字 → 1..=255、表にない文字は 0 + UTF-8)
pub struct ReadingCoder {
    chars: Vec<char>,
    index: HashMap<char, u8>,
}

impl ReadingCoder {
    pub fn new(chars: Vec<char>) -> Self {
        let index = chars
            .iter()
            .take(READING_TABLE_MAX)
            .enumerate()
            .map(|(i, &c)| (c, (i + 1) as u8))
            .collect();
        Self { chars, index }
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    /// 1 文字を符号化して `out` に追加する
    pub fn encode_char(&self, c: char, out: &mut Vec<u8>) {
        match self.index.get(&c) {
            Some(&code) => out.push(code),
            None => {
                out.push(0);
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }

    pub fn encode(&self, s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(s.len());
        for c in s.chars() {
            self.encode_char(c, &mut out);
        }
        out
    }

    /// 符号列を復号する (trie の DFS 用)。不正な列は置換文字になる
    pub fn decode(&self, codes: &[u8]) -> String {
        let mut s = String::new();
        let mut i = 0;
        while i < codes.len() {
            let b = codes[i];
            if b == 0 {
                let len = utf8_len(codes.get(i + 1).copied().unwrap_or(0));
                let end = (i + 1 + len).min(codes.len());
                s.push_str(&String::from_utf8_lossy(&codes[i + 1..end]));
                i = end;
            } else {
                s.push(
                    self.chars
                        .get(b as usize - 1)
                        .copied()
                        .unwrap_or('\u{FFFD}'),
                );
                i += 1;
            }
        }
        s
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// 表層の可変長符号化表
pub struct SurfaceCoder {
    chars: Vec<char>,
    index: HashMap<char, u32>,
}

impl SurfaceCoder {
    /// 文字数が `SURFACE_TABLE_MAX` を超えるとエラー
    pub fn new(chars: Vec<char>) -> Result<Self, String> {
        if chars.len() > SURFACE_TABLE_MAX {
            return Err(format!(
                "too many surface chars: {} > {SURFACE_TABLE_MAX}",
                chars.len()
            ));
        }
        let index = chars
            .iter()
            .enumerate()
            .map(|(i, &c)| (c, i as u32))
            .collect();
        Ok(Self { chars, index })
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    /// 文字列を符号化して `out` に追加する。表にない文字はエラー
    pub fn encode(&self, s: &str, out: &mut Vec<u8>) -> Result<(), String> {
        for c in s.chars() {
            let idx = *self
                .index
                .get(&c)
                .ok_or_else(|| format!("char {c:?} not in surface table"))?
                as usize;
            if idx < SURFACE_SINGLE {
                out.push(idx as u8);
            } else if idx < SURFACE_SINGLE + SURFACE_DOUBLE {
                let rel = idx - SURFACE_SINGLE;
                out.push((SURFACE_SINGLE + rel / 256) as u8);
                out.push((rel % 256) as u8);
            } else {
                let rel = idx - SURFACE_SINGLE - SURFACE_DOUBLE;
                out.push(255);
                out.push((rel / 256) as u8);
                out.push((rel % 256) as u8);
            }
        }
        Ok(())
    }

    /// 符号列を復号する。不正な列は置換文字になる (panic しない)
    pub fn decode(&self, codes: &[u8]) -> String {
        let mut s = String::with_capacity(codes.len() * 2);
        let mut i = 0;
        while i < codes.len() {
            let b = codes[i] as usize;
            let (idx, used) = if b < SURFACE_SINGLE {
                (b, 1)
            } else if b < 255 {
                let lo = codes.get(i + 1).copied().unwrap_or(0) as usize;
                (SURFACE_SINGLE + (b - SURFACE_SINGLE) * 256 + lo, 2)
            } else {
                let hi = codes.get(i + 1).copied().unwrap_or(0) as usize;
                let lo = codes.get(i + 2).copied().unwrap_or(0) as usize;
                (SURFACE_SINGLE + SURFACE_DOUBLE + hi * 256 + lo, 3)
            };
            s.push(self.chars.get(idx).copied().unwrap_or('\u{FFFD}'));
            i += used;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_roundtrip() {
        let chars = vec!['あ', '漢', 'a', '🎉'];
        let mut out = Vec::new();
        write_table(&chars, &mut out);
        assert_eq!(read_table(&out).unwrap(), chars);
        assert!(read_table(&out[..5]).is_err());
        let mut bad = out.clone();
        bad[4..8].copy_from_slice(&0xD800u32.to_le_bytes());
        assert!(read_table(&bad).is_err());
    }

    #[test]
    fn test_frequency_table_order_and_limit() {
        let t = frequency_table(["ああい", "いう", "う", "う"].into_iter(), 2);
        assert_eq!(t, vec!['う', 'あ']);
    }

    #[test]
    fn test_reading_coder_escape() {
        let coder = ReadingCoder::new(vec!['き', 'ょ', 'う']);
        assert_eq!(coder.encode("きょう"), vec![1, 2, 3]);
        let enc = coder.encode("き漢");
        assert_eq!(enc[0], 1);
        assert_eq!(enc[1], 0);
        assert_eq!(&enc[2..], "漢".as_bytes());
        assert_eq!(coder.decode(&enc), "き漢");
        assert_eq!(coder.decode(&[0, 0xE6]), "\u{FFFD}");
        assert_eq!(coder.decode(&[200]), "\u{FFFD}");
    }

    #[test]
    fn test_reading_coder_255_limit() {
        let chars: Vec<char> = (0..300)
            .map(|i| char::from_u32(0x3041 + i).unwrap())
            .collect();
        let coder = ReadingCoder::new(chars.clone());
        let enc = coder.encode(&chars[254].to_string());
        assert_eq!(enc, vec![255]);
        let enc = coder.encode(&chars[255].to_string());
        assert_eq!(enc[0], 0);
    }

    #[test]
    fn test_surface_coder_tiers() {
        let chars: Vec<char> = (0..SURFACE_SINGLE + SURFACE_DOUBLE + 10)
            .map(|i| char::from_u32(0x4E00 + i as u32).unwrap())
            .collect();
        let coder = SurfaceCoder::new(chars.clone()).unwrap();
        let samples = [
            0,
            SURFACE_SINGLE - 1,
            SURFACE_SINGLE,
            SURFACE_SINGLE + 300,
            SURFACE_SINGLE + SURFACE_DOUBLE - 1,
            SURFACE_SINGLE + SURFACE_DOUBLE,
            SURFACE_SINGLE + SURFACE_DOUBLE + 9,
        ];
        let text: String = samples.iter().map(|&i| chars[i]).collect();
        let mut out = Vec::new();
        coder.encode(&text, &mut out).unwrap();
        assert_eq!(out.len(), 1 + 1 + 2 + 2 + 2 + 3 + 3);
        assert_eq!(coder.decode(&out), text);
        assert!(coder.encode("ん", &mut out).is_err());
        assert_eq!(coder.decode(&[255, 1, 0]), "\u{FFFD}");
        assert_eq!(coder.decode(&[255, 0, 20]), "\u{FFFD}");
    }

    #[test]
    fn test_surface_coder_rejects_oversized_table() {
        let chars: Vec<char> = (0..SURFACE_TABLE_MAX as u32 + 1)
            .filter_map(|i| char::from_u32(0x10000 + i))
            .collect();
        assert!(SurfaceCoder::new(chars).is_err());
    }
}
