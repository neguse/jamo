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

fn ipadic_available() -> bool {
    std::path::Path::new("data/ipadic").exists()
}

fn start_server(port: u16) -> ServerGuard {
    let child = Command::new(env!("CARGO_BIN_EXE_jamo-cli"))
        .args([
            "serve",
            "--dict",
            "data/ipadic",
            "--port",
            &port.to_string(),
        ])
        .spawn()
        .expect("failed to start server");

    // サーバー起動待ち
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            return ServerGuard { child };
        }
    }
    panic!("server did not start within 5 seconds");
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
    if !ipadic_available() {
        return;
    }
    let _server = start_server(18091);
    let body = http_get("http://127.0.0.1:18091/health");
    assert_eq!(body, "ok");
}

#[test]
fn test_convert_api() {
    if !ipadic_available() {
        return;
    }
    let _server = start_server(18092);
    // q=わたしはがくせいです (URL encoded)
    let body = http_get(
        "http://127.0.0.1:18092/convert?q=%E3%82%8F%E3%81%9F%E3%81%97%E3%81%AF%E3%81%8C%E3%81%8F%E3%81%9B%E3%81%84%E3%81%A7%E3%81%99",
    );
    let json: serde_json::Value = serde_json::from_str(&body).expect("invalid json");
    let result = json["result"].as_array().expect("result should be array");
    assert!(!result.is_empty(), "result should not be empty");

    // セグメントの candidates を全部つなげると「私は学生です」になるはず
    let text: String = result
        .iter()
        .map(|seg| seg["candidates"][0].as_str().unwrap_or(""))
        .collect();
    assert_eq!(text, "私は学生です");
}
