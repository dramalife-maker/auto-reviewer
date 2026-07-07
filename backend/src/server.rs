use axum::extract::{Path, State};
use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::Error;
use crate::reports;
use crate::runs::{self, RunRow};
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

#[derive(Serialize)]
pub struct RunStatusResponse {
    pub id: i64,
    pub trigger: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub project_total: Option<i64>,
    pub project_skipped: i64,
}

impl From<RunRow> for RunStatusResponse {
    fn from(row: RunRow) -> Self {
        Self {
            id: row.id,
            trigger: row.trigger,
            status: row.status,
            started_at: row.started_at,
            finished_at: row.finished_at,
            project_total: row.project_total,
            project_skipped: row.project_skipped,
        }
    }
}

pub fn router(state: AppState) -> Router {
    let cors_origins = state.config.cors_allow_origins().to_vec();
    let router = Router::new()
        .route("/health", get(health))
        .route("/api/runs", post(create_run))
        .route("/api/runs/{id}", get(get_run))
        .route("/api/people", get(list_people))
        .route("/api/people/{id}/reports/latest", get(latest_reports))
        .route("/api/reports/{id}/read", patch(mark_report_read))
        .with_state(state);

    apply_cors(router, &cors_origins)
}

fn apply_cors(router: Router, origins: &[String]) -> Router {
    if origins.is_empty() {
        return router;
    }

    let allow_origin = if origins.iter().any(|origin| origin == "*") {
        AllowOrigin::any()
    } else {
        let allowed: Vec<http::HeaderValue> = origins
            .iter()
            .filter_map(|origin| origin.parse().ok())
            .collect();
        if allowed.is_empty() {
            return router;
        }
        AllowOrigin::list(allowed)
    };

    router.layer(
        CorsLayer::new()
            .allow_origin(allow_origin)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE]),
    )
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

    Ok((StatusCode::CREATED, Json(CreateRunResponse { run_id })))
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Result<Json<RunStatusResponse>, ApiError> {
    let run = runs::get_run(&state.pool, run_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(Error::NotFound)?;
    Ok(Json(run.into()))
}

async fn list_people(State(state): State<AppState>) -> Result<Json<Vec<reports::PersonListItem>>, ApiError> {
    let people = reports::list_people(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(people))
}

async fn latest_reports(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
) -> Result<Json<reports::LatestReportsResponse>, ApiError> {
    let response = reports::latest_reports_for_person(&state.pool, person_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(Error::NotFound)?;
    Ok(Json(response))
}

async fn mark_report_read(
    State(state): State<AppState>,
    Path(report_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let updated = reports::mark_report_read(&state.pool, report_id)
        .await
        .map_err(ApiError::from)?;
    if updated {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::from(Error::NotFound))
    }
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
            Error::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.0.to_string()).into_response()
    }
}
