use crate::dict::POS_CONTENT;
use crate::lattice::Lattice;

/// 接続コスト: 前ノードの right_id と 次ノードの left_id から接続コストを返す
pub trait ConnectionCost {
    fn cost(&self, right_id: u16, left_id: u16) -> i32;
}

/// 仮の接続コスト (全ペア固定値)
pub struct SimpleConnectionCost;

impl ConnectionCost for SimpleConnectionCost {
    fn cost(&self, _right_id: u16, _left_id: u16) -> i32 {
        // Iter 1 では品詞接続をまだ考慮しない。固定ペナルティ。
        0
    }
}

/// 変換結果の1単語
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ipc", derive(serde::Serialize, serde::Deserialize))]
pub struct Segment {
    /// 表層形 (最良候補)
    pub surface: String,
    /// 読み (入力のかな列の該当部分)
    pub yomi: String,
    /// 変換候補 (コスト順、最良が先頭)
    pub candidates: Vec<String>,
    /// 品詞タイプ (POS_CONTENT or POS_FUNCTION)
    pub pos_type: u8,
}

/// 文節 (自立語 + 付属語のグループ)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ipc", derive(serde::Serialize, serde::Deserialize))]
pub struct Clause {
    /// 表層形 (文節全体)
    pub surface: String,
    /// 読み (文節全体)
    pub yomi: String,
    /// 変換候補 (文節単位)
    pub candidates: Vec<String>,
}

/// 変換結果
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ipc", derive(serde::Serialize, serde::Deserialize))]
pub struct ConvertResult {
    /// 単語単位のセグメント
    pub segments: Vec<Segment>,
    /// 文節単位のグループ
    pub clauses: Vec<Clause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    EmptyForNonEmptyInput,
    NotStrictlyAscending { index: usize },
    OutOfBounds { index: usize, value: usize },
    NotCharBoundary { index: usize, value: usize },
    MissingInputEnd { expected: usize, actual: usize },
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoundaryError::EmptyForNonEmptyInput => {
                write!(f, "boundaries are empty for non-empty input")
            }
            BoundaryError::NotStrictlyAscending { index } => {
                write!(f, "boundary at index {index} is not strictly ascending")
            }
            BoundaryError::OutOfBounds { index, value } => {
                write!(f, "boundary at index {index} is out of bounds: {value}")
            }
            BoundaryError::NotCharBoundary { index, value } => {
                write!(
                    f,
                    "boundary at index {index} is not a UTF-8 char boundary: {value}"
                )
            }
            BoundaryError::MissingInputEnd { expected, actual } => {
                write!(f, "last boundary must be {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for BoundaryError {}

/// 文節境界を検証する。
pub fn validate_boundaries(input: &str, boundaries: &[usize]) -> Result<(), BoundaryError> {
    if input.is_empty() {
        return match boundaries {
            [] | [0] => Ok(()),
            _ => Err(BoundaryError::MissingInputEnd {
                expected: 0,
                actual: *boundaries.last().unwrap_or(&usize::MAX),
            }),
        };
    }

    if boundaries.is_empty() {
        return Err(BoundaryError::EmptyForNonEmptyInput);
    }

    let mut prev = 0;
    for (i, &boundary) in boundaries.iter().enumerate() {
        if boundary <= prev {
            return Err(BoundaryError::NotStrictlyAscending { index: i });
        }
        if boundary > input.len() {
            return Err(BoundaryError::OutOfBounds {
                index: i,
                value: boundary,
            });
        }
        if !input.is_char_boundary(boundary) {
            return Err(BoundaryError::NotCharBoundary {
                index: i,
                value: boundary,
            });
        }
        prev = boundary;
    }

    let last = *boundaries.last().expect("checked non-empty");
    if last != input.len() {
        return Err(BoundaryError::MissingInputEnd {
            expected: input.len(),
            actual: last,
        });
    }

    Ok(())
}

pub fn clause_boundaries(clauses: &[Clause]) -> Vec<usize> {
    let mut pos = 0;
    clauses
        .iter()
        .map(|c| {
            pos += c.yomi.len();
            pos
        })
        .collect()
}

/// Viterbi デコードを実行して最適パスを返す
pub fn decode(lattice: &mut Lattice, input: &str, conn: &dyn ConnectionCost) -> ConvertResult {
    decode_nbest(lattice, input, conn, 1)
}

/// Viterbi デコードを実行して N-best 候補付きの結果を返す
pub fn decode_nbest(
    lattice: &mut Lattice,
    input: &str,
    conn: &dyn ConnectionCost,
    n: usize,
) -> ConvertResult {
    let input_len = lattice.input_len;

    // 前進: 各ノードの最適コストと最適前ノードを計算
    for pos in 0..=input_len {
        let node_indices: Vec<usize> = lattice.ends_at[pos].clone();
        for &node_idx in &node_indices {
            if node_idx == lattice.bos_index() {
                continue;
            }

            let node_start = lattice.nodes[node_idx].start;
            let node_left_id = lattice.nodes[node_idx].left_id;
            let node_word_cost = lattice.nodes[node_idx].word_cost;

            let prev_indices: Vec<usize> = lattice.ends_at[node_start].clone();
            let mut best_cost = i64::MAX;
            let mut best_prev = None;

            for &prev_idx in &prev_indices {
                let prev_total = lattice.nodes[prev_idx].total_cost;
                if prev_total == i64::MAX {
                    continue;
                }
                let prev_right_id = lattice.nodes[prev_idx].right_id;
                let c = conn.cost(prev_right_id, node_left_id) as i64;
                let total = prev_total.saturating_add(c).saturating_add(node_word_cost);
                if total < best_cost {
                    best_cost = total;
                    best_prev = Some(prev_idx);
                }
            }

            if best_cost < lattice.nodes[node_idx].total_cost {
                lattice.nodes[node_idx].total_cost = best_cost;
                lattice.nodes[node_idx].prev = best_prev;
            }
        }
    }

    // 後退: EOS から BOS へ最適パスをたどる
    let mut path = Vec::new();
    let mut current = lattice.eos_index();
    while let Some(prev) = lattice.nodes[current].prev {
        if prev == lattice.bos_index() {
            break;
        }
        path.push(prev);
        current = prev;
    }
    path.reverse();

    // パスからセグメントを生成 (N-best 候補付き)
    let n = n.max(1);
    let segments: Vec<Segment> = path
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let node = &lattice.nodes[idx];
            let surface = node.surface.clone();
            let yomi = input[node.start..node.end].to_string();

            // 同じ (start, end) 区間の他の候補を文脈込みコストで集める:
            // total_cost (BOS からこのノードまでの最良コスト) + 次パスノードへの接続コスト
            let next_left_id = path
                .get(i + 1)
                .map(|&next| lattice.nodes[next].left_id)
                .unwrap_or(0); // 末尾は EOS (id 0)
            let mut others: Vec<(i64, String)> = lattice.ends_at[node.end]
                .iter()
                .filter_map(|&other_idx| {
                    let other = &lattice.nodes[other_idx];
                    if other.start == node.start
                        && other.end == node.end
                        && !other.surface.is_empty()
                        && other.surface != surface
                    {
                        let score = other
                            .total_cost
                            .saturating_add(conn.cost(other.right_id, next_left_id) as i64);
                        Some((score, other.surface.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            // 同一表層は最良スコアのみ残してからスコア順に並べる
            others.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
            others.dedup_by(|a, b| a.1 == b.1);
            others.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

            // Viterbi 最適を先頭に、残りを追加
            let mut candidates = vec![surface.clone()];
            candidates.extend(others.into_iter().take(n - 1).map(|(_, s)| s));

            Segment {
                surface,
                yomi,
                candidates,
                pos_type: node.pos_type,
            }
        })
        .collect();

    let clauses = group_clauses(&segments);
    ConvertResult { segments, clauses }
}

/// 単語セグメントを文節にグルーピングする
/// 自立語 (POS_CONTENT) が来たら新しい文節を開始
/// 付属語 (POS_FUNCTION) は直前の文節に付く
pub fn group_clauses(segments: &[Segment]) -> Vec<Clause> {
    if segments.is_empty() {
        return Vec::new();
    }

    // 文節境界を決定: 各セグメントがどの文節に属するか
    let mut clause_groups: Vec<Vec<usize>> = Vec::new();

    for (i, seg) in segments.iter().enumerate() {
        if seg.pos_type == POS_CONTENT || clause_groups.is_empty() {
            // 自立語 or 最初のセグメント → 新文節開始
            clause_groups.push(vec![i]);
        } else {
            // 付属語 → 現在の文節に追加
            clause_groups.last_mut().unwrap().push(i);
        }
    }

    // 各文節の候補を生成
    clause_groups
        .iter()
        .map(|indices| {
            let surface: String = indices
                .iter()
                .map(|&i| segments[i].surface.as_str())
                .collect();
            let yomi: String = indices.iter().map(|&i| segments[i].yomi.as_str()).collect();

            // 候補生成: ベスト + 各単語の代替候補を使ったバリエーション
            let mut candidates = vec![surface.clone()];
            for (pos, &seg_idx) in indices.iter().enumerate() {
                let seg = &segments[seg_idx];
                for alt in seg.candidates.iter().skip(1) {
                    let variant: String = indices
                        .iter()
                        .enumerate()
                        .map(|(j, &idx)| {
                            if j == pos {
                                alt.as_str()
                            } else {
                                segments[idx].surface.as_str()
                            }
                        })
                        .collect();
                    if !candidates.contains(&variant) {
                        candidates.push(variant);
                    }
                }
            }

            Clause {
                surface,
                yomi,
                candidates,
            }
        })
        .collect()
}

/// 文節境界を調整する (伸縮)
/// clause_idx: 伸縮する文節のインデックス
/// delta: +N で文字数分伸ばす (右に境界を移動), -N で縮める (左に境界を移動)
/// 戻り値: 新しい文節境界バイト位置のリスト (不正な操作の場合は None)
pub fn resize_boundaries(
    input: &str,
    boundaries: &[usize],
    clause_idx: usize,
    delta: i32,
) -> Option<Vec<usize>> {
    if validate_boundaries(input, boundaries).is_err()
        || clause_idx >= boundaries.len()
        || boundaries.is_empty()
    {
        return None;
    }

    // 最後の境界は入力末尾に固定
    if clause_idx == boundaries.len() - 1 {
        return None; // 最後の文節は伸縮不可
    }

    let current = boundaries[clause_idx];
    let next = boundaries[clause_idx + 1];
    let prev_start = if clause_idx > 0 {
        boundaries[clause_idx - 1]
    } else {
        0
    };

    let new_pos = if delta > 0 {
        // 伸ばす: 次の文節の先頭から delta 文字分を奪う
        let remaining = &input[current..next];
        let char_count = remaining.chars().count();
        let move_count = delta as usize;
        if move_count >= char_count {
            // 次の文節を完全に吸収 → 境界を削除して統合
            let mut new_boundaries = boundaries.to_vec();
            new_boundaries.remove(clause_idx);
            return validate_boundaries(input, &new_boundaries)
                .ok()
                .map(|()| new_boundaries);
        }
        remaining
            .char_indices()
            .nth(move_count)
            .map(|(offset, _)| current + offset)
            .unwrap_or(next)
    } else {
        // 縮める: 現在の文節の末尾から |delta| 文字分を次の文節に渡す
        let clause_reading = &input[prev_start..current];
        let char_positions: Vec<usize> = clause_reading
            .char_indices()
            .map(|(offset, _)| prev_start + offset)
            .collect();
        let move_count = (-delta) as usize;
        if move_count >= char_positions.len() {
            return None; // 現在の文節がなくなる
        }
        char_positions[char_positions.len() - move_count]
    };

    let mut new_boundaries = boundaries.to_vec();
    new_boundaries[clause_idx] = new_pos;
    validate_boundaries(input, &new_boundaries)
        .ok()
        .map(|()| new_boundaries)
}

/// 文節境界を指定して変換する
/// boundaries: 各文節の読みの終了バイト位置 (最後は input.len())
pub fn try_convert_with_boundaries(
    input: &str,
    boundaries: &[usize],
    dict: &dyn crate::dict::Dictionary,
    conn: &dyn ConnectionCost,
    n: usize,
) -> Result<ConvertResult, BoundaryError> {
    validate_boundaries(input, boundaries)?;

    let mut all_segments = Vec::new();
    let mut all_clauses = Vec::new();

    let mut start = 0;
    for &end in boundaries {
        let sub_input = &input[start..end];
        if sub_input.is_empty() {
            start = end;
            continue;
        }

        let mut lat = Lattice::build(sub_input, dict);
        let sub_result = decode_nbest(&mut lat, sub_input, conn, n);

        // この範囲の全セグメントを1文節にまとめる
        let surface: String = sub_result
            .segments
            .iter()
            .map(|s| s.surface.as_str())
            .collect();
        let yomi = sub_input.to_string();

        // 文節候補: 内部文節の組み合わせから生成
        let mut candidates = vec![surface.clone()];
        for clause in &sub_result.clauses {
            for alt in clause.candidates.iter().skip(1) {
                if !candidates.contains(alt) {
                    candidates.push(alt.clone());
                }
            }
        }

        // セグメントの yomi をオフセット調整 (元の入力での位置に)
        let mut seg_start = start;
        for mut seg in sub_result.segments {
            let seg_end = seg_start + seg.yomi.len();
            seg.yomi = input[seg_start..seg_end].to_string();
            seg_start = seg_end;
            all_segments.push(seg);
        }

        all_clauses.push(Clause {
            surface,
            yomi,
            candidates,
        });

        start = end;
    }

    Ok(ConvertResult {
        segments: all_segments,
        clauses: all_clauses,
    })
}

pub fn convert_with_boundaries(
    input: &str,
    boundaries: &[usize],
    dict: &dyn crate::dict::Dictionary,
    conn: &dyn ConnectionCost,
    n: usize,
) -> ConvertResult {
    try_convert_with_boundaries(input, boundaries, dict, conn, n).unwrap_or_else(|_| {
        ConvertResult {
            segments: Vec::new(),
            clauses: Vec::new(),
        }
    })
}

#[cfg(test)]
mod tests;
