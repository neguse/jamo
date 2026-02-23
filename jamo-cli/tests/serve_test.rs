use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

struct ServerGuard {
    child: Child,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_server(port: u16) -> ServerGuard {
    let child = Command::new(env!("CARGO_BIN_EXE_jamo-cli"))
        .args(["serve", "--port", &port.to_string()])
        .spawn()
        .expect("failed to start server");

    // サーバー起動待ち
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(100));
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            return ServerGuard { child };
        }
    }
    panic!("server did not start within 2 seconds");
}

fn http_get(url: &str) -> String {
    let output = Command::new("curl")
        .args(["-s", url])
        .output()
        .expect("curl failed");
    String::from_utf8(output.stdout).expect("invalid utf8")
}

#[test]
fn test_health() {
    let _server = start_server(18091);
    let body = http_get("http://127.0.0.1:18091/health");
    assert_eq!(body, "ok");
}

#[test]
fn test_convert_api() {
    let _server = start_server(18092);
    let body = http_get("http://127.0.0.1:18092/convert?q=%E3%81%8D%E3%82%87%E3%81%86%E3%81%AF");
    // q=きょうは (URL encoded)
    let json: serde_json::Value = serde_json::from_str(&body).expect("invalid json");
    let result = json["result"].as_array().expect("result should be array");
    assert!(!result.is_empty(), "result should not be empty");

    // 最初のセグメントが「きょう」→「今日」
    assert_eq!(result[0]["yomi"], "きょう");
    assert_eq!(result[0]["candidates"][0], "今日");
}
