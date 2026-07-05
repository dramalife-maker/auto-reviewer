use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::runs;
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub data_dir: String,
}

#[derive(Deserialize)]
pub struct CreateRunRequest {
    pub trigger: String,
}

#[derive(Serialize)]
pub struct CreateRunResponse {
    pub run_id: i64,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/runs", post(create_run))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        data_dir: state.config.data_dir().display().to_string(),
    })
}

async fn create_run(
    State(state): State<AppState>,
    Json(body): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    if body.trigger != "manual_all" {
        return Err(ApiError::from(Error::UnsupportedRunTrigger(body.trigger)));
    }

    let run_id = runs::create_manual_all_run(&state.pool)
        .await
        .map_err(ApiError::from)?;

    if let Some(worker) = &state.worker {
        worker.wake();
    }

    Ok((
        StatusCode::CREATED,
        Json(CreateRunResponse { run_id }),
    ))
}

#[derive(Debug)]
struct ApiError(Error);

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            Error::RunConflict => StatusCode::CONFLICT,
            Error::UnsupportedRunTrigger(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.0.to_string()).into_response()
    }
}
