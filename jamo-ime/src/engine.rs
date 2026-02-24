/// IME エンジン (IPC クライアント)
///
/// Named Pipe (Windows) / Unix Socket (Linux) 経由で jamo-server と通信する。
/// 辞書ロードは不要。サーバー側で辞書を保持する。
use std::cell::RefCell;
use std::io::{BufRead, BufReader, BufWriter, Write};

use jamo_core::ipc::{IpcRequest, IpcResponse, PredictEntry};
use jamo_core::viterbi::{Clause, ConvertResult, Segment};

/// IPC 接続
struct IpcConnection {
    reader: BufReader<std::fs::File>,
    writer: BufWriter<std::fs::File>,
}

impl IpcConnection {
    /// Named Pipe / Unix Socket に接続
    fn connect() -> Result<Self, String> {
        #[cfg(windows)]
        let file = {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(jamo_core::ipc::PIPE_NAME)
                .map_err(|e| format!("pipe connect failed: {e}"))?
        };

        #[cfg(unix)]
        let file = {
            use std::os::unix::net::UnixStream;
            let stream =
                UnixStream::connect(jamo_core::ipc::SOCKET_PATH).map_err(|e| e.to_string())?;
            // UnixStream → File は直接変換できないので、TcpStream のように使う
            // 実際は Windows Named Pipe のみが本番パス
            // Linux ではテストのために File ではなく UnixStream を使う
            drop(stream);
            return Err("Unix socket not supported via File; use test helpers".to_string());
        };

        #[cfg(not(any(windows, unix)))]
        return Err("unsupported platform".to_string());

        #[cfg(windows)]
        {
            let reader =
                BufReader::new(file.try_clone().map_err(|e| format!("clone failed: {e}"))?);
            let writer = BufWriter::new(file);
            Ok(Self { reader, writer })
        }
    }

    /// リクエスト送信 → レスポンス受信
    fn send(&mut self, request: &IpcRequest) -> Result<IpcResponse, String> {
        let json = serde_json::to_string(request).map_err(|e| e.to_string())?;
        self.writer
            .write_all(json.as_bytes())
            .map_err(|e| e.to_string())?;
        self.writer.write_all(b"\n").map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())?;

        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(line.trim()).map_err(|e| e.to_string())
    }
}

/// IME エンジン (IPC クライアントラッパー)
pub struct ImeEngine {
    conn: RefCell<Option<IpcConnection>>,
}

impl ImeEngine {
    /// IPC サーバーに接続を試みる
    pub fn connect() -> Self {
        let conn = IpcConnection::connect().ok();
        if conn.is_some() {
            eprintln!("jamo-ime: connected to IPC server");
        } else {
            eprintln!("jamo-ime: IPC server not available, will retry on first request");
        }
        Self {
            conn: RefCell::new(conn),
        }
    }

    /// リクエストを送信 (接続がなければ再接続を試みる)
    fn send_request(&self, request: &IpcRequest) -> Option<IpcResponse> {
        let mut conn_ref = self.conn.borrow_mut();

        // 未接続なら再接続試行
        if conn_ref.is_none() {
            *conn_ref = IpcConnection::connect().ok();
        }

        if let Some(conn) = conn_ref.as_mut() {
            match conn.send(request) {
                Ok(response) => return Some(response),
                Err(_) => {
                    // 接続断: 次回再接続
                    *conn_ref = None;
                    return None;
                }
            }
        }

        None
    }

    /// かな漢字変換
    pub fn convert(&self, hiragana: &str, n: usize) -> ConvertResult {
        let request = IpcRequest::Convert {
            hiragana: hiragana.to_string(),
            n,
        };

        match self.send_request(&request) {
            Some(IpcResponse::Convert { result }) => result,
            _ => fallback_result(hiragana),
        }
    }

    /// 文節伸縮して再変換
    pub fn resize(
        &self,
        input: &str,
        boundaries: &[usize],
        clause: usize,
        delta: i32,
        n: usize,
    ) -> Option<ConvertResult> {
        let request = IpcRequest::Resize {
            input: input.to_string(),
            boundaries: boundaries.to_vec(),
            clause,
            delta,
            n,
        };

        match self.send_request(&request) {
            Some(IpcResponse::Convert { result }) => Some(result),
            _ => None,
        }
    }

    /// 予測変換
    pub fn predict(&self, prefix: &str, limit: usize) -> Vec<PredictEntry> {
        let request = IpcRequest::Predict {
            prefix: prefix.to_string(),
            limit,
        };

        match self.send_request(&request) {
            Some(IpcResponse::Predict { entries }) => entries,
            _ => vec![],
        }
    }
}

/// IPC サーバー未接続時のフォールバック (ひらがなをそのまま返す)
fn fallback_result(hiragana: &str) -> ConvertResult {
    ConvertResult {
        segments: vec![Segment {
            surface: hiragana.to_string(),
            yomi: hiragana.to_string(),
            candidates: vec![hiragana.to_string()],
            pos_type: 0,
        }],
        clauses: vec![Clause {
            surface: hiragana.to_string(),
            yomi: hiragana.to_string(),
            candidates: vec![hiragana.to_string()],
        }],
    }
}
