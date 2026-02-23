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
    result: Vec<ClauseResponse>,
}

#[derive(Serialize)]
struct ClauseResponse {
    yomi: String,
    candidates: Vec<String>,
}

async fn handle_convert(
    State(dict): State<Arc<IpaDict>>,
    Query(params): Query<ConvertParams>,
) -> impl IntoResponse {
    let result = jamo_core::convert_nbest(&params.q, dict.as_ref(), dict.as_ref(), params.n);

    let clauses: Vec<ClauseResponse> = result
        .clauses
        .iter()
        .map(|c| ClauseResponse {
            yomi: c.yomi.clone(),
            candidates: c.candidates.clone(),
        })
        .collect();

    Json(ConvertResponse { result: clauses })
}

#[derive(Deserialize)]
struct ResizeParams {
    q: String,
    /// カンマ区切りの文節境界バイト位置
    boundaries: String,
    /// 伸縮する文節のインデックス
    clause: usize,
    /// +N で伸ばす, -N で縮める
    delta: i32,
    #[serde(default = "default_n")]
    n: usize,
}

async fn handle_resize(
    State(dict): State<Arc<IpaDict>>,
    Query(params): Query<ResizeParams>,
) -> impl IntoResponse {
    let boundaries: Vec<usize> = params
        .boundaries
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let new_boundaries =
        match jamo_core::resize_boundaries(&params.q, &boundaries, params.clause, params.delta) {
            Some(b) => b,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid resize operation"})),
                );
            }
        };

    let result = jamo_core::convert_with_boundaries(
        &params.q,
        &new_boundaries,
        dict.as_ref(),
        dict.as_ref(),
        params.n,
    );

    let clauses: Vec<ClauseResponse> = result
        .clauses
        .iter()
        .map(|c| ClauseResponse {
            yomi: c.yomi.clone(),
            candidates: c.candidates.clone(),
        })
        .collect();

    let boundaries_str: Vec<String> = new_boundaries.iter().map(|b| b.to_string()).collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "result": clauses,
            "boundaries": boundaries_str,
        })),
    )
}

async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn run(port: u16, dict: Arc<IpaDict>) {
    let app = Router::new()
        .route("/convert", get(handle_convert))
        .route("/resize", get(handle_resize))
        .route("/health", get(handle_health))
        .with_state(dict);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("jamo server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
