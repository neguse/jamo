use wasm_bindgen::prelude::*;

use crate::ipadic::IpaDict;

/// WASM 向けエンジン: バイナリ辞書を保持して変換を行う
#[wasm_bindgen]
pub struct JamoEngine {
    dict: IpaDict,
}

#[wasm_bindgen]
impl JamoEngine {
    /// バイナリ辞書データからエンジンを生成
    #[wasm_bindgen(constructor)]
    pub fn new(dict_bytes: &[u8]) -> Result<JamoEngine, JsError> {
        let mut cursor = std::io::Cursor::new(dict_bytes);
        let dict = IpaDict::load_binary_from_reader(&mut cursor)
            .map_err(|e| JsError::new(&format!("failed to load dict: {e}")))?;
        Ok(JamoEngine { dict })
    }

    /// かな漢字変換 (JSON 文字列を返す)
    pub fn convert(&self, input: &str, n: usize) -> String {
        let result = crate::convert_nbest(input, &self.dict, &self.dict, n);

        let clauses: Vec<serde_json::Value> = result
            .clauses
            .iter()
            .map(|c| {
                serde_json::json!({
                    "yomi": c.yomi,
                    "candidates": c.candidates,
                })
            })
            .collect();

        let boundaries = crate::clause_boundaries(&result.clauses);

        serde_json::json!({
            "result": clauses,
            "boundaries": boundaries,
        })
        .to_string()
    }

    /// 予測変換: プレフィクスで始まる候補を返す (JSON 文字列)
    pub fn predict(&self, prefix: &str, limit: usize) -> String {
        let entries = self.dict.prefix_search(prefix, limit);
        let mut seen = std::collections::HashSet::new();
        let candidates: Vec<serde_json::Value> = entries
            .into_iter()
            .filter(|e| seen.insert(e.surface.clone()))
            .map(|e| {
                serde_json::json!({
                    "surface": e.surface,
                    "cost": e.cost,
                })
            })
            .collect();

        serde_json::json!({ "candidates": candidates }).to_string()
    }

    /// 文節伸縮 + 再変換 (JSON 文字列を返す)
    pub fn resize(
        &self,
        input: &str,
        boundaries_json: &str,
        clause_idx: usize,
        delta: i32,
        n: usize,
    ) -> Result<String, JsError> {
        let boundaries: Vec<usize> = serde_json::from_str(boundaries_json)
            .map_err(|e| JsError::new(&format!("invalid boundaries: {e}")))?;

        let new_boundaries = crate::resize_boundaries(input, &boundaries, clause_idx, delta)
            .ok_or_else(|| JsError::new("invalid resize operation"))?;

        let result =
            crate::try_convert_with_boundaries(input, &new_boundaries, &self.dict, &self.dict, n)
                .map_err(|e| JsError::new(&e.to_string()))?;

        let clauses: Vec<serde_json::Value> = result
            .clauses
            .iter()
            .map(|c| {
                serde_json::json!({
                    "yomi": c.yomi,
                    "candidates": c.candidates,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "result": clauses,
            "boundaries": new_boundaries,
        })
        .to_string())
    }
}
