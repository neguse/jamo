pub mod dict;
pub mod lattice;
pub mod viterbi;

use dict::Dictionary;
use viterbi::{ConvertResult, SimpleConnectionCost};

/// かな文字列を変換する
pub fn convert(input: &str, dict: &dyn Dictionary) -> ConvertResult {
    let mut lat = lattice::Lattice::build(input, dict);
    let conn = SimpleConnectionCost;
    viterbi::decode(&mut lat, input, &conn)
}

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
