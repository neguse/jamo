/// IME IPC サーバー (Named Pipe / Unix Socket)
///
/// jamo-ime DLL からの IPC リクエストを受け付け、変換結果を返す。
/// プロトコル: NDJSON (1行1メッセージ)
use std::sync::Arc;

use jamo_core::ipadic::IpaDict;
use jamo_core::ipc::{IpcRequest, IpcResponse, PredictEntry};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

/// IPC リクエストを処理して IPC レスポンスを返す
fn process_request(request: &IpcRequest, dict: &IpaDict) -> IpcResponse {
    match request {
        IpcRequest::Convert { hiragana, n } => {
            let result = jamo_core::convert_nbest(hiragana, dict, dict, *n);
            IpcResponse::Convert { result }
        }
        IpcRequest::Resize {
            input,
            boundaries,
            clause,
            delta,
            n,
        } => match jamo_core::resize_boundaries(input, boundaries, *clause, *delta) {
            Some(new_boundaries) => {
                match jamo_core::try_convert_with_boundaries(input, &new_boundaries, dict, dict, *n)
                {
                    Ok(result) => IpcResponse::Convert { result },
                    Err(e) => IpcResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            None => IpcResponse::Error {
                message: "invalid resize operation".to_string(),
            },
        },
        IpcRequest::Predict { prefix, limit } => {
            let entries: Vec<PredictEntry> = dict
                .prefix_search(prefix, *limit)
                .into_iter()
                .map(|e| PredictEntry {
                    surface: e.surface,
                    cost: e.cost,
                })
                .collect();
            IpcResponse::Predict { entries }
        }
    }
}

/// 1接続を処理するループ
async fn handle_connection<S: AsyncRead + AsyncWrite + Unpin>(stream: S, dict: Arc<IpaDict>) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        }

        let response = match serde_json::from_str::<IpcRequest>(line.trim()) {
            Ok(request) => process_request(&request, &dict),
            Err(e) => IpcResponse::Error {
                message: e.to_string(),
            },
        };

        let json = match serde_json::to_string(&response) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("serialize error: {e}");
                break;
            }
        };

        if writer.write_all(json.as_bytes()).await.is_err() {
            break;
        }
        if writer.write_all(b"\n").await.is_err() {
            break;
        }
        if writer.flush().await.is_err() {
            break;
        }
    }
}

/// IME IPC サーバーを起動
#[cfg(unix)]
pub async fn run(dict: Arc<IpaDict>) {
    let path = jamo_core::ipc::SOCKET_PATH;
    // 前回のソケットファイルを削除
    let _ = std::fs::remove_file(path);

    let listener = tokio::net::UnixListener::bind(path).expect("failed to bind Unix socket");
    eprintln!("jamo IME server listening on {path}");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let d = dict.clone();
                tokio::spawn(handle_connection(stream, d));
            }
            Err(e) => {
                eprintln!("accept error: {e}");
            }
        }
    }
}

#[cfg(windows)]
pub async fn run(dict: Arc<IpaDict>) {
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe_name = jamo_core::ipc::PIPE_NAME;
    eprintln!("jamo IME server listening on {pipe_name}");

    let mut first = true;
    loop {
        let server = match ServerOptions::new()
            .first_pipe_instance(first)
            .create(pipe_name)
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("failed to create pipe: {e}");
                return;
            }
        };
        first = false;

        if let Err(e) = server.connect().await {
            eprintln!("pipe connect error: {e}");
            continue;
        }

        let d = dict.clone();
        tokio::spawn(handle_connection(server, d));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jamo_core::ipc::IpcRequest;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn test_ime_server_convert() {
        let dict =
            IpaDict::load(std::path::Path::new("../data/ipadic")).expect("need ipadic for test");
        let dict = Arc::new(dict);

        let socket_path = "/tmp/jamo-ime-test.sock";
        let _ = std::fs::remove_file(socket_path);

        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("failed to bind test socket");

        let server_dict = dict.clone();
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, server_dict).await;
        });

        // クライアント側
        let stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .expect("failed to connect");
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);

        // convert リクエスト送信
        let req = IpcRequest::Convert {
            hiragana: "きょう".to_string(),
            n: 3,
        };
        let mut json = serde_json::to_string(&req).unwrap();
        json.push('\n');
        writer.write_all(json.as_bytes()).await.unwrap();
        writer.flush().await.unwrap();

        // レスポンス受信
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let resp: IpcResponse = serde_json::from_str(line.trim()).unwrap();

        match resp {
            IpcResponse::Convert { result } => {
                assert!(!result.clauses.is_empty());
            }
            other => panic!("unexpected response: {other:?}"),
        }

        // 接続を閉じる → サーバーループ終了
        drop(writer);
        drop(reader);
        let _ = server_handle.await;
        let _ = std::fs::remove_file(socket_path);
    }

    #[tokio::test]
    async fn test_ime_server_predict() {
        let dict =
            IpaDict::load(std::path::Path::new("../data/ipadic")).expect("need ipadic for test");
        let dict = Arc::new(dict);

        let socket_path = "/tmp/jamo-ime-test-predict.sock";
        let _ = std::fs::remove_file(socket_path);

        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("failed to bind test socket");

        let server_dict = dict.clone();
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, server_dict).await;
        });

        let stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .expect("failed to connect");
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);

        // predict リクエスト送信
        let req = IpcRequest::Predict {
            prefix: "きょう".to_string(),
            limit: 5,
        };
        let mut json = serde_json::to_string(&req).unwrap();
        json.push('\n');
        writer.write_all(json.as_bytes()).await.unwrap();
        writer.flush().await.unwrap();

        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let resp: IpcResponse = serde_json::from_str(line.trim()).unwrap();

        match resp {
            IpcResponse::Predict { entries } => {
                // ipadic には「きょう」で始まるエントリがあるはず
                assert!(!entries.is_empty());
            }
            other => panic!("unexpected response: {other:?}"),
        }

        drop(writer);
        drop(reader);
        let _ = server_handle.await;
        let _ = std::fs::remove_file(socket_path);
    }
}
