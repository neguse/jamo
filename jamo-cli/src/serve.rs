use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use jamo_core::ipadic::IpaDict;
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

async fn handle_convert(
    State(dict): State<Arc<IpaDict>>,
    Query(params): Query<ConvertParams>,
) -> impl IntoResponse {
    let result = jamo_core::convert_with_conn(&params.q, dict.as_ref(), dict.as_ref());

    let _ = params.n; // N-best は後のIter

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

pub async fn run(port: u16, dict: Arc<IpaDict>) {
    let app = Router::new()
        .route("/convert", get(handle_convert))
        .route("/health", get(handle_health))
        .with_state(dict);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("jamo server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
