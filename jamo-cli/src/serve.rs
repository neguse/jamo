use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
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
    boundaries: Vec<usize>,
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
    let boundaries = jamo_core::clause_boundaries(&result.clauses);

    let clauses: Vec<ClauseResponse> = result
        .clauses
        .iter()
        .map(|c| ClauseResponse {
            yomi: c.yomi.clone(),
            candidates: c.candidates.clone(),
        })
        .collect();

    Json(ConvertResponse {
        result: clauses,
        boundaries,
    })
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
) -> Response {
    let boundaries = match parse_boundaries_param(&params.boundaries) {
        Ok(boundaries) => boundaries,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": message})),
            )
                .into_response();
        }
    };

    let new_boundaries =
        match jamo_core::resize_boundaries(&params.q, &boundaries, params.clause, params.delta) {
            Some(b) => b,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid resize operation"})),
                )
                    .into_response();
            }
        };

    let result = match jamo_core::try_convert_with_boundaries(
        &params.q,
        &new_boundaries,
        dict.as_ref(),
        dict.as_ref(),
        params.n,
    ) {
        Ok(result) => result,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let clauses: Vec<ClauseResponse> = result
        .clauses
        .iter()
        .map(|c| ClauseResponse {
            yomi: c.yomi.clone(),
            candidates: c.candidates.clone(),
        })
        .collect();

    Json(ConvertResponse {
        result: clauses,
        boundaries: new_boundaries,
    })
    .into_response()
}

#[derive(Deserialize)]
struct PredictParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

#[derive(Serialize)]
struct PredictResponse {
    candidates: Vec<PredictCandidate>,
}

#[derive(Serialize)]
struct PredictCandidate {
    surface: String,
    cost: i16,
}

async fn handle_predict(
    State(dict): State<Arc<IpaDict>>,
    Query(params): Query<PredictParams>,
) -> impl IntoResponse {
    let candidates = dict
        .prefix_search(&params.q, params.limit)
        .into_iter()
        .map(|e| PredictCandidate {
            surface: e.surface,
            cost: e.cost,
        })
        .collect();
    Json(PredictResponse { candidates })
}

fn parse_boundaries_param(value: &str) -> Result<Vec<usize>, String> {
    if value.trim().is_empty() {
        return Err("boundaries must not be empty".to_string());
    }

    value
        .split(',')
        .map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return Err("empty boundary value".to_string());
            }
            s.parse::<usize>()
                .map_err(|_| format!("invalid boundary value: {s}"))
        })
        .collect()
}

async fn handle_method_not_allowed() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(serde_json::json!({"error": "method not allowed"})),
    )
}

async fn handle_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "not found"})),
    )
}

async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn run(port: u16, dict: Arc<IpaDict>) {
    let app = Router::new()
        .route(
            "/convert",
            get(handle_convert).fallback(handle_method_not_allowed),
        )
        .route(
            "/resize",
            get(handle_resize).fallback(handle_method_not_allowed),
        )
        .route(
            "/predict",
            get(handle_predict).fallback(handle_method_not_allowed),
        )
        .route(
            "/health",
            get(handle_health).fallback(handle_method_not_allowed),
        )
        .fallback(handle_not_found)
        .with_state(dict);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("jamo server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");
    axum::serve(listener, app).await.expect("server error");
}
