/// IME IPC プロトコル (JSON over Named Pipe / Unix Socket)
///
/// jamo-cli (サーバー) と jamo-ime (クライアント) 間の通信プロトコル。
/// 1行1メッセージの NDJSON (Newline Delimited JSON) 形式。
use serde::{Deserialize, Serialize};

use crate::viterbi::ConvertResult;

/// Named Pipe 名 (Windows)
#[cfg(windows)]
pub const PIPE_NAME: &str = r"\\.\pipe\jamo-ime";

/// Unix ソケットパス (Linux/macOS, テスト用)
#[cfg(unix)]
pub const SOCKET_PATH: &str = "/tmp/jamo-ime.sock";

/// IPC リクエスト
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    /// かな漢字変換
    #[serde(rename = "convert")]
    Convert { hiragana: String, n: usize },
    /// 文節伸縮して再変換
    #[serde(rename = "resize")]
    Resize {
        input: String,
        boundaries: Vec<usize>,
        clause: usize,
        delta: i32,
        n: usize,
    },
    /// 予測変換
    #[serde(rename = "predict")]
    Predict { prefix: String, limit: usize },
}

/// IPC レスポンス
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    /// 変換結果 (convert / resize 共通)
    #[serde(rename = "convert")]
    Convert { result: ConvertResult },
    /// 予測変換結果
    #[serde(rename = "predict")]
    Predict { entries: Vec<PredictEntry> },
    /// エラー
    #[serde(rename = "error")]
    Error { message: String },
}

/// 予測変換エントリ (辞書 Entry の簡略版)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictEntry {
    pub surface: String,
    pub cost: i16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viterbi::{Clause, Segment};

    #[test]
    fn test_request_serialize() {
        let req = IpcRequest::Convert {
            hiragana: "きょう".to_string(),
            n: 5,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"convert\""));
        assert!(json.contains("\"hiragana\":\"きょう\""));
    }

    #[test]
    fn test_request_deserialize() {
        let json = r#"{"type":"convert","hiragana":"きょう","n":5}"#;
        let req: IpcRequest = serde_json::from_str(json).unwrap();
        match req {
            IpcRequest::Convert { hiragana, n } => {
                assert_eq!(hiragana, "きょう");
                assert_eq!(n, 5);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_resize_request_roundtrip() {
        let req = IpcRequest::Resize {
            input: "きょうは".to_string(),
            boundaries: vec![9, 12],
            clause: 0,
            delta: 1,
            n: 5,
        };
        let json = serde_json::to_string(&req).unwrap();
        let req2: IpcRequest = serde_json::from_str(&json).unwrap();
        match req2 {
            IpcRequest::Resize {
                input,
                boundaries,
                clause,
                delta,
                n,
            } => {
                assert_eq!(input, "きょうは");
                assert_eq!(boundaries, vec![9, 12]);
                assert_eq!(clause, 0);
                assert_eq!(delta, 1);
                assert_eq!(n, 5);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_response_convert_roundtrip() {
        let result = ConvertResult {
            segments: vec![Segment {
                surface: "今日".to_string(),
                yomi: "きょう".to_string(),
                candidates: vec!["今日".to_string(), "京".to_string()],
                pos_type: 0,
            }],
            clauses: vec![Clause {
                surface: "今日".to_string(),
                yomi: "きょう".to_string(),
                candidates: vec!["今日".to_string(), "京".to_string()],
            }],
        };
        let resp = IpcResponse::Convert { result };
        let json = serde_json::to_string(&resp).unwrap();
        let resp2: IpcResponse = serde_json::from_str(&json).unwrap();
        match resp2 {
            IpcResponse::Convert { result } => {
                assert_eq!(result.clauses.len(), 1);
                assert_eq!(result.clauses[0].surface, "今日");
                assert_eq!(result.clauses[0].candidates.len(), 2);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_response_predict_roundtrip() {
        let resp = IpcResponse::Predict {
            entries: vec![
                PredictEntry {
                    surface: "京都".to_string(),
                    cost: -100,
                },
                PredictEntry {
                    surface: "今日".to_string(),
                    cost: -50,
                },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let resp2: IpcResponse = serde_json::from_str(&json).unwrap();
        match resp2 {
            IpcResponse::Predict { entries } => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].surface, "京都");
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn test_response_error() {
        let resp = IpcResponse::Error {
            message: "invalid resize".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        let resp2: IpcResponse = serde_json::from_str(&json).unwrap();
        match resp2 {
            IpcResponse::Error { message } => {
                assert_eq!(message, "invalid resize");
            }
            _ => panic!("unexpected variant"),
        }
    }
}
