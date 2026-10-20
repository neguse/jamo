// このファイルの規則のパースとマッチ (Rule / BoundaryRules) は mozc の src/converter/segmenter.cc と src/converter/gen_boundary_data.py の移植で、tests の DEF は src/data/rules/boundary.def の抜粋である。
// 出典: https://github.com/google/mozc
// これらの部分には以下の上流ライセンスが適用される。
//
// Copyright 2010-2021, Google Inc.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
//     * Redistributions of source code must retain the above copyright
// notice, this list of conditions and the following disclaimer.
//     * Redistributions in binary form must reproduce the above
// copyright notice, this list of conditions and the following disclaimer
// in the documentation and/or other materials provided with the
// distribution.
//     * Neither the name of Google Inc. nor the names of its
// contributors may be used to endorse or promote products derived from
// this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! 文節境界ペナルティ (mozc の boundary.def)
//!
//! 変換キーの先頭に来るノードには品詞 (left_id) ごとの PREFIX ペナルティ、
//! 末尾に来るノードには right_id ごとの SUFFIX ペナルティを単語コストに足す。
//! 「いく」単独で非自立動詞 (〜ていく) が勝つ、「かく」で接頭詞「各」が勝つ、
//! といった単文節の誤変換を、辞書コストと接続行列を変えずに抑える。
//! 本家 mozc の converter/segmenter.cc (GetPrefixPenalty / GetSuffixPenalty) と
//! converter/gen_boundary_data.py の移植。
//!
//! boundary.def の 1 行は `PREFIX|SUFFIX <品詞パターン> <コスト>`。パターンは id.def の
//! 素性 (カンマ区切り) に先頭からマッチし、`*` は 1 フィールド (カンマ以外 1 文字以上)、
//! `(a|b|c)` は選択。パターンがカンマで終わらなければ、その直後がカンマか末尾である
//! こと。同じ種類の中で最初にマッチした規則のコストを使い、どれにも合わなければ 0。

/// パターンの要素
#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Lit(char),
    /// `*`: カンマ以外の 1 文字以上
    Field,
    /// `(a|b)`: どれか 1 つ
    Alt(Vec<String>),
}

/// 1 規則 (パース済みパターン + コスト)
#[derive(Debug, Clone)]
pub struct Rule {
    pieces: Vec<Piece>,
    /// パターンがカンマで終わる (= 以降は任意)
    open_end: bool,
    cost: u16,
}

impl Rule {
    fn parse(pattern: &str, cost: u16) -> Rule {
        let mut pieces = Vec::new();
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '*' => pieces.push(Piece::Field),
                '(' => {
                    let end = chars[i..]
                        .iter()
                        .position(|&c| c == ')')
                        .map(|p| i + p)
                        .unwrap_or(chars.len());
                    let body: String = chars[i + 1..end].iter().collect();
                    pieces.push(Piece::Alt(body.split('|').map(str::to_string).collect()));
                    i = end;
                }
                c => pieces.push(Piece::Lit(c)),
            }
            i += 1;
        }
        Rule {
            pieces,
            open_end: pattern.ends_with(','),
            cost,
        }
    }

    /// 素性文字列の先頭からマッチするか
    fn matches(&self, feature: &str) -> bool {
        let chars: Vec<char> = feature.chars().collect();
        self.match_from(0, &chars, 0)
    }

    fn match_from(&self, piece: usize, chars: &[char], pos: usize) -> bool {
        if piece == self.pieces.len() {
            return self.open_end || pos == chars.len() || chars[pos] == ',';
        }
        match &self.pieces[piece] {
            Piece::Lit(c) => {
                chars.get(pos) == Some(c) && self.match_from(piece + 1, chars, pos + 1)
            }
            Piece::Field => {
                // 1 文字以上、カンマ以外。長さを変えてバックトラック
                let mut end = pos;
                while end < chars.len() && chars[end] != ',' {
                    end += 1;
                    if self.match_from(piece + 1, chars, end) {
                        return true;
                    }
                }
                false
            }
            Piece::Alt(options) => options.iter().any(|opt| {
                let o: Vec<char> = opt.chars().collect();
                chars.len() >= pos + o.len()
                    && chars[pos..pos + o.len()] == o[..]
                    && self.match_from(piece + 1, chars, pos + o.len())
            }),
        }
    }
}

/// パース済みの boundary.def
#[derive(Debug, Clone, Default)]
pub struct BoundaryRules {
    prefix: Vec<Rule>,
    suffix: Vec<Rule>,
}

impl BoundaryRules {
    /// boundary.def の内容をパースする。不正な行はエラー
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut rules = BoundaryRules::default();
        for (lineno, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 3 {
                return Err(format!("boundary.def:{}: 3 fields expected", lineno + 1));
            }
            let cost: u16 = f[2]
                .parse()
                .map_err(|e| format!("boundary.def:{}: cost: {e}", lineno + 1))?;
            let rule = Rule::parse(f[1], cost);
            match f[0] {
                "PREFIX" => rules.prefix.push(rule),
                "SUFFIX" => rules.suffix.push(rule),
                other => {
                    return Err(format!(
                        "boundary.def:{}: unknown label {other}",
                        lineno + 1
                    ));
                }
            }
        }
        Ok(rules)
    }

    fn cost(rules: &[Rule], feature: &str) -> u16 {
        rules
            .iter()
            .find(|r| r.matches(feature))
            .map(|r| r.cost)
            .unwrap_or(0)
    }

    /// 素性に対する (PREFIX ペナルティ, SUFFIX ペナルティ)
    pub fn penalties(&self, feature: &str) -> (u16, u16) {
        (
            Self::cost(&self.prefix, feature),
            Self::cost(&self.suffix, feature),
        )
    }

    /// id.def (`<id> <素性>` 行) の全 ID 分のペナルティ表。ID の欠番は 0
    pub fn table_from_id_def(&self, id_def: &str) -> Vec<(u16, u16)> {
        let mut table = Vec::new();
        for line in id_def.lines() {
            let mut it = line.splitn(2, ' ');
            let (Some(id), Some(feat)) = (it.next(), it.next()) else {
                continue;
            };
            let Ok(id) = id.parse::<usize>() else {
                continue;
            };
            if id >= table.len() {
                table.resize(id + 1, (0, 0));
            }
            table[id] = self.penalties(feat.trim());
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: &str = "\
# comment
SUFFIX 動詞,自立,*,*,*,基本形        0
PREFIX 名詞,接尾,一般,               1000
PREFIX 名詞,接尾,                    1000
PREFIX 名詞,(数|固有名詞|副詞可能|代名詞|一般|サ変接続), 0
SUFFIX 名詞,一般,*,*,*,*,*,気        500
SUFFIX 名詞,(固有名詞|副詞可能|代名詞|一般|サ変接続|接尾), 300
SUFFIX 接頭詞,名詞接続,*,*,*,*,*,(各|約|超|当|同|全|元|新|等)  0
PREFIX 接頭詞,名詞接続               250
PREFIX 動詞,自立                     100
PREFIX 動詞,非自立                    4000
SUFFIX 接頭詞, 4000
SUFFIX 助詞,副助詞／並立助詞／終助詞,*,*,*,*,か	750
";

    #[test]
    fn test_rules_match_like_mozc() {
        let r = BoundaryRules::parse(DEF).unwrap();
        // 非自立動詞は先頭で 4000
        assert_eq!(
            r.penalties("動詞,非自立,*,*,五段・カ行促音便,基本形,いく"),
            (4000, 0)
        );
        assert_eq!(
            r.penalties("動詞,自立,*,*,五段・カ行促音便,基本形,行く"),
            (100, 0)
        );
        // 7 フィールドの素性に 8 フィールドのパターンは当たらない → 接頭詞, 4000
        assert_eq!(r.penalties("接頭詞,名詞接続,*,*,*,*,各"), (250, 4000));
        assert_eq!(r.penalties("接頭詞,名詞接続,*,*,*,*,*,各"), (250, 0));
        // 名詞,一般: PREFIX は (…|一般) で 0、SUFFIX は 気 でなければ 300
        assert_eq!(r.penalties("名詞,一般,*,*,*,*,*"), (0, 300));
        assert_eq!(r.penalties("名詞,一般,*,*,*,*,*,気"), (0, 500));
        assert_eq!(r.penalties("名詞,接尾,一般,*,*,*,*"), (1000, 300));
        // パターン末尾がカンマでなければ直後はカンマか末尾
        assert_eq!(r.penalties("動詞,自立"), (100, 0));
        assert_eq!(r.penalties("動詞,自立性,*"), (0, 0));
        // 全角スラッシュ等はリテラル
        assert_eq!(
            r.penalties("助詞,副助詞／並立助詞／終助詞,*,*,*,*,か"),
            (0, 750)
        );
        assert_eq!(
            r.penalties("助詞,副助詞／並立助詞／終助詞,*,*,*,*,も"),
            (0, 0)
        );
        // どれにも合わなければ 0
        assert_eq!(r.penalties("BOS/EOS,*,*,*,*,*,*"), (0, 0));
    }

    #[test]
    fn test_field_wildcard_needs_one_char() {
        let r = BoundaryRules::parse("PREFIX 名詞,*,一般 5\n").unwrap();
        assert_eq!(r.penalties("名詞,接尾,一般"), (5, 0));
        assert_eq!(r.penalties("名詞,,一般"), (0, 0));
        assert_eq!(r.penalties("名詞,接尾,一般人"), (0, 0));
    }

    #[test]
    fn test_table_from_id_def() {
        let r = BoundaryRules::parse(DEF).unwrap();
        let table = r.table_from_id_def(
            "0 BOS/EOS,*,*,*,*,*,*\n2 動詞,非自立,*,*,一段,基本形,いる\n1 名詞,一般,*,*,*,*,*\n",
        );
        assert_eq!(table, vec![(0, 0), (0, 300), (4000, 0)]);
    }

    #[test]
    fn test_parse_errors() {
        assert!(BoundaryRules::parse("PREFIX 名詞\n").is_err());
        assert!(BoundaryRules::parse("MIDDLE 名詞, 10\n").is_err());
        assert!(BoundaryRules::parse("PREFIX 名詞, -1\n").is_err());
        assert!(BoundaryRules::parse("").unwrap().prefix.is_empty());
    }
}
