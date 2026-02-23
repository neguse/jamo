pub mod bitvec;
pub mod dict;
pub mod ipadic;
pub mod lattice;
pub mod louds;
pub mod viterbi;

use dict::Dictionary;
use viterbi::{ConvertResult, SimpleConnectionCost};

use viterbi::ConnectionCost;

/// かな文字列を変換する (SimpleConnectionCost)
pub fn convert(input: &str, dict: &dyn Dictionary) -> ConvertResult {
    let mut lat = lattice::Lattice::build(input, dict);
    let conn = SimpleConnectionCost;
    viterbi::decode(&mut lat, input, &conn)
}

/// かな文字列を変換する (接続コスト指定)
pub fn convert_with_conn(
    input: &str,
    dict: &dyn Dictionary,
    conn: &dyn ConnectionCost,
) -> ConvertResult {
    let mut lat = lattice::Lattice::build(input, dict);
    viterbi::decode(&mut lat, input, conn)
}

/// かな文字列を変換する (N-best 候補付き)
pub fn convert_nbest(
    input: &str,
    dict: &dyn Dictionary,
    conn: &dyn ConnectionCost,
    n: usize,
) -> ConvertResult {
    let mut lat = lattice::Lattice::build(input, dict);
    viterbi::decode_nbest(&mut lat, input, conn, n)
}

/// 文節グルーピングのみ (テスト用に公開)
pub use viterbi::group_clauses;

/// 文節境界を調整する
pub use viterbi::resize_boundaries;

/// 文節境界を指定して変換する
pub use viterbi::convert_with_boundaries;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::HashMapDict;

    #[test]
    fn test_convert_kyouha() {
        let dict = HashMapDict::test_dict();
        let result = convert("きょうは", &dict);
        let text: String = result.segments.iter().map(|s| s.surface.as_str()).collect();
        assert_eq!(text, "今日は");
    }

    #[test]
    fn test_convert_desu() {
        let dict = HashMapDict::test_dict();
        let result = convert("です", &dict);
        assert_eq!(result.segments.len(), 1);
        assert_eq!(result.segments[0].surface, "です");
    }
}
