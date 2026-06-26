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

fn http_status(method: &str, url: &str) -> u16 {
    let output = Command::new("curl")
        .args([
            "-s",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "-X",
            method,
            url,
        ])
        .output()
        .expect("curl failed");
    String::from_utf8(output.stdout)
        .expect("invalid utf8")
        .parse()
        .expect("invalid status")
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
    let boundaries = json["boundaries"]
        .as_array()
        .expect("boundaries should be array");
    assert!(!boundaries.is_empty(), "boundaries should not be empty");
    assert!(
        boundaries.iter().all(|b| b.as_u64().is_some()),
        "boundaries should be numeric: {boundaries:?}"
    );

    // セグメントの candidates を全部つなげると「私は学生です」になるはず
    let text: String = result
        .iter()
        .map(|seg| seg["candidates"][0].as_str().unwrap_or(""))
        .collect();
    assert_eq!(text, "私は学生です");
}

#[test]
fn test_predict_api() {
    if !ipadic_available() {
        return;
    }
    let _server = start_server(18093);
    let body = http_get("http://127.0.0.1:18093/predict?q=%E3%81%8D%E3%82%87%E3%81%86");
    let json: serde_json::Value = serde_json::from_str(&body).expect("invalid json");
    let candidates = json["candidates"]
        .as_array()
        .expect("candidates should be array");
    assert!(!candidates.is_empty(), "candidates should not be empty");
}

#[test]
fn test_resize_rejects_invalid_boundary() {
    if !ipadic_available() {
        return;
    }
    let _server = start_server(18094);
    let status = http_status(
        "GET",
        "http://127.0.0.1:18094/resize?q=%E3%81%8D%E3%82%87%E3%81%86%E3%81%AF&boundaries=1,12&clause=0&delta=1",
    );
    assert_eq!(status, 400);
}

#[test]
fn test_wrong_method_is_405() {
    if !ipadic_available() {
        return;
    }
    let _server = start_server(18095);
    let status = http_status(
        "POST",
        "http://127.0.0.1:18095/convert?q=%E3%81%8D%E3%82%87%E3%81%86",
    );
    assert_eq!(status, 405);
}
