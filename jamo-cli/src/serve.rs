use axum::{
    Router,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct ConvertParams {
    q: String,
    #[serde(default = "default_n")]
    n: usize,
}

fn default_n() -> usize {
    5
}

#[derive(Serialize)]
struct ConvertResponse {
    result: Vec<SegmentResponse>,
}

#[derive(Serialize)]
struct SegmentResponse {
    yomi: String,
    candidates: Vec<String>,
}

async fn handle_convert(Query(params): Query<ConvertParams>) -> impl IntoResponse {
    let dict = jamo_core::dict::HashMapDict::test_dict();
    let result = jamo_core::convert(&params.q, &dict);

    let _ = params.n; // N-best は Iter 2 で対応

    let segments: Vec<SegmentResponse> = result
        .segments
        .iter()
        .map(|s| SegmentResponse {
            yomi: s.yomi.clone(),
            candidates: vec![s.surface.clone()],
        })
        .collect();

    Json(ConvertResponse { result: segments })
}

async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn run(port: u16) {
    let app = Router::new()
        .route("/convert", get(handle_convert))
        .route("/health", get(handle_health));

    let addr = format!("0.0.0.0:{port}");
    eprintln!("jamo server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
