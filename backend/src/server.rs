use axum::extract::{Path, Query, State};
use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::dashboard;
use crate::error::Error;
use crate::identity;
use crate::mr_reviews::{self, AgentTurnResponse, MrReviewListItem, PublishResponse};
use crate::pending_items;
use crate::person_trends;
use crate::projects;
use crate::reports;
use crate::runs::{self, RunRow};
use crate::schedule::{self, ScheduleConfigResponse, ScheduleUpdateInput};
use crate::state::AppState;
use crate::worktree;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub data_dir: String,
}

#[derive(Deserialize)]
pub struct CreateRunRequest {
    pub trigger: String,
    pub project_name: Option<String>,
}

#[derive(Serialize)]
pub struct CreateRunResponse {
    pub run_id: i64,
}

#[derive(Deserialize)]
pub struct MrScanQuery {
    pub force: Option<String>,
}

#[derive(Serialize)]
pub struct ReloadProjectsResponse {
    pub total: usize,
    pub healthy: usize,
    pub unhealthy: usize,
    pub projects: Vec<projects::ProjectHealth>,
}

#[derive(Serialize)]
pub struct RunProjectStatusResponse {
    pub name: String,
    pub state: String,
    pub error: Option<String>,
}

impl From<runs::RunProjectStatusRow> for RunProjectStatusResponse {
    fn from(row: runs::RunProjectStatusRow) -> Self {
        Self {
            name: row.name,
            state: row.state,
            error: row.error,
        }
    }
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
    pub projects: Vec<RunProjectStatusResponse>,
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
            projects: Vec::new(),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let cors_origins = state.config.cors_allow_origins().to_vec();
    let router = Router::new()
        .route("/health", get(health))
        .route("/api/runs", post(create_run))
        .route("/api/runs/{id}", get(get_run))
        .route("/api/dashboard", get(get_dashboard))
        .route("/api/schedule", get(get_schedule).patch(update_schedule))
        .route("/api/people", get(list_people).post(create_person))
        .route("/api/people/{id}", get(get_person).patch(rename_person))
        .route("/api/people/{id}/reports/latest", get(latest_reports))
        .route("/api/people/{id}/trends", get(person_trends))
        .route("/api/people/{id}/pending-items", get(list_pending_items))
        .route("/api/pending-items/{id}", patch(resolve_pending_item))
        .route("/api/people/{id}/identities", get(list_person_identities).post(bind_person_identity))
        .route("/api/people/{id}/identities/{identity_id}", delete(unbind_person_identity))
        .route("/api/unmatched-authors", get(list_unmatched_authors))
        .route("/api/reports/{id}/read", patch(mark_report_read))
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/projects/reload", post(reload_projects))
        .route("/api/projects/{id}/mr-scan", post(mr_scan))
        .route("/api/projects/{name}", put(update_project).delete(delete_project))
        .route("/api/mr-reviews", get(list_mr_reviews))
        .route("/api/mr-reviews/{id}", patch(update_mr_review))
        .route("/api/mr-reviews/{id}/publish", post(publish_mr_review))
        .route("/api/mr-reviews/{id}/ignore", post(ignore_mr_review))
        .route("/api/mr-reviews/{id}/agent-turn", post(agent_turn_mr_review))
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
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
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
    let run_id = match body.trigger.as_str() {
        "manual_all" => runs::create_manual_all_run(&state.pool).await?,
        "manual_project" => {
            let project_name = body
                .project_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or(Error::InvalidProjectConfig(
                    "manual_project requires project_name".into(),
                ))?;
            runs::create_manual_project_run(&state.pool, project_name).await?
        }
        other => return Err(ApiError::from(Error::UnsupportedRunTrigger(other.to_string()))),
    };

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
    let projects = runs::list_run_project_statuses(&state.pool, run_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(RunStatusResponse {
        id: run.id,
        trigger: run.trigger,
        status: run.status,
        started_at: run.started_at,
        finished_at: run.finished_at,
        project_total: run.project_total,
        project_skipped: run.project_skipped,
        projects: projects.into_iter().map(Into::into).collect(),
    }))
}

async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<projects::ProjectListResponse>, ApiError> {
    let response = projects::list_project_details(&state.pool, state.config.data_dir())
        .await
        .map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn create_project(
    State(state): State<AppState>,
    Json(body): Json<projects::ProjectInput>,
) -> Result<(StatusCode, Json<projects::ProjectListItem>), ApiError> {
    let project = projects::create_project(&state.pool, state.config.data_dir(), body)
        .await
        .map_err(ApiError::from)?;
    let resolved = projects::load_resolved_from_db(&state.pool)
        .await
        .map_err(ApiError::from)?;
    worktree::provision_all(&state.pool, &resolved).await;
    let refreshed = projects::list_project_details(&state.pool, state.config.data_dir())
        .await
        .map_err(ApiError::from)?
        .projects
        .into_iter()
        .find(|item| item.name == project.name)
        .unwrap_or(project);
    Ok((StatusCode::CREATED, Json(refreshed)))
}

async fn update_project(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<projects::ProjectUpdateInput>,
) -> Result<Json<projects::ProjectListItem>, ApiError> {
    let project = projects::update_project(&state.pool, state.config.data_dir(), &name, body)
        .await
        .map_err(ApiError::from)?;
    let resolved = projects::load_resolved_from_db(&state.pool)
        .await
        .map_err(ApiError::from)?;
    worktree::provision_all(&state.pool, &resolved).await;
    let refreshed = projects::list_project_details(&state.pool, state.config.data_dir())
        .await
        .map_err(ApiError::from)?
        .projects
        .into_iter()
        .find(|item| item.name == project.name)
        .unwrap_or(project);
    Ok(Json(refreshed))
}

async fn delete_project(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    projects::delete_project(&state.pool, &name)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reload_projects(
    State(state): State<AppState>,
) -> Result<Json<ReloadProjectsResponse>, ApiError> {
    let resolved = projects::load_resolved_from_db(&state.pool)
        .await
        .map_err(ApiError::from)?;

    worktree::provision_all(&state.pool, &resolved).await;

    let projects = projects::list_projects(&state.pool)
        .await
        .map_err(ApiError::from)?;
    let healthy = projects
        .iter()
        .filter(|project| project.health == "healthy")
        .count();
    let unhealthy = projects.len() - healthy;

    Ok(Json(ReloadProjectsResponse {
        total: projects.len(),
        healthy,
        unhealthy,
        projects,
    }))
}

async fn get_dashboard(
    State(state): State<AppState>,
) -> Result<Json<dashboard::DashboardResponse>, ApiError> {
    let response = dashboard::load_dashboard(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn get_schedule(
    State(state): State<AppState>,
) -> Result<Json<ScheduleConfigResponse>, ApiError> {
    let response = schedule::get_schedule_config_response(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn update_schedule(
    State(state): State<AppState>,
    Json(body): Json<ScheduleUpdateInput>,
) -> Result<Json<ScheduleConfigResponse>, ApiError> {
    let response = schedule::update_schedule_config(&state.pool, body)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn list_people(State(state): State<AppState>) -> Result<Json<Vec<reports::PersonListItem>>, ApiError> {
    let people = reports::list_people(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(people))
}

#[derive(Deserialize)]
struct CreatePersonRequest {
    display_name: String,
}

#[derive(Serialize)]
struct CreatePersonResponse {
    id: i64,
    display_name: String,
}

async fn create_person(
    State(state): State<AppState>,
    Json(body): Json<CreatePersonRequest>,
) -> Result<(StatusCode, Json<CreatePersonResponse>), ApiError> {
    let display_name = body.display_name.trim().to_string();
    let person_id = identity::create_person(&state.pool, &display_name)
        .await
        .map_err(ApiError::from)?;
    Ok((
        StatusCode::CREATED,
        Json(CreatePersonResponse {
            id: person_id,
            display_name,
        }),
    ))
}

async fn get_person(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
) -> Result<Json<identity::PersonDetail>, ApiError> {
    let detail = identity::get_person_detail(&state.pool, person_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(detail))
}

#[derive(Deserialize)]
struct RenamePersonRequest {
    display_name: String,
}

async fn rename_person(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
    Json(body): Json<RenamePersonRequest>,
) -> Result<Json<identity::PersonDetail>, ApiError> {
    let detail = identity::rename_person(
        &state.pool,
        state.config.data_dir(),
        person_id,
        &body.display_name,
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(detail))
}

async fn list_unmatched_authors(
    State(state): State<AppState>,
) -> Result<Json<Vec<identity::UnmatchedAuthorItem>>, ApiError> {
    let items = identity::list_unmatched_authors(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(items))
}

#[derive(Deserialize)]
struct BindIdentityRequest {
    kind: String,
    value: String,
    label: Option<String>,
}

async fn bind_person_identity(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
    Json(body): Json<BindIdentityRequest>,
) -> Result<StatusCode, ApiError> {
    identity::bind_identity(
        &state.pool,
        person_id,
        &body.kind,
        &body.value,
        body.label.as_deref(),
    )
    .await
    .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_person_identities(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
) -> Result<Json<Vec<identity::IdentityItem>>, ApiError> {
    let items = identity::list_identities_for_person(&state.pool, person_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(items))
}

async fn unbind_person_identity(
    State(state): State<AppState>,
    Path((person_id, identity_id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    identity::unbind_identity(&state.pool, person_id, identity_id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
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

async fn person_trends(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
) -> Result<Json<person_trends::PersonTrendsResponse>, ApiError> {
    let response = person_trends::load_trends(
        &state.pool,
        state.config.data_dir(),
        person_id,
    )
    .await
    .map_err(ApiError::from)?
    .ok_or(Error::NotFound)?;
    Ok(Json(response))
}

#[derive(Deserialize)]
struct PendingItemListQuery {
    status: Option<String>,
}

async fn list_pending_items(
    State(state): State<AppState>,
    Path(person_id): Path<i64>,
    Query(query): Query<PendingItemListQuery>,
) -> Result<Json<Vec<pending_items::PendingItem>>, ApiError> {
    let items = pending_items::list_pending_items_for_person(
        &state.pool,
        person_id,
        query.status.as_deref(),
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(items))
}

async fn resolve_pending_item(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
    Json(body): Json<pending_items::ResolvePendingItemInput>,
) -> Result<Json<pending_items::PendingItem>, ApiError> {
    let item = pending_items::resolve_pending_item(
        &state.pool,
        state.config.data_dir(),
        item_id,
        body,
    )
    .await
    .map_err(ApiError::from)?;
    Ok(Json(item))
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

#[derive(Deserialize)]
struct MrReviewListQuery {
    status: Option<String>,
}

async fn list_mr_reviews(
    State(state): State<AppState>,
    Query(query): Query<MrReviewListQuery>,
) -> Result<Json<Vec<MrReviewListItem>>, ApiError> {
    let status = query.status.as_deref();
    if let Some(status) = status {
        if !matches!(status, "draft" | "published" | "ignored") {
            return Err(ApiError::from(Error::InvalidProjectConfig(
                "status must be draft, published, or ignored".into(),
            )));
        }
    }
    let items = mr_reviews::list_mr_reviews(&state.pool, status)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(items))
}

#[derive(Deserialize)]
struct UpdateMrReviewRequest {
    draft_body: String,
}

async fn update_mr_review(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateMrReviewRequest>,
) -> Result<StatusCode, ApiError> {
    mr_reviews::update_draft(&state.pool, id, &body.draft_body)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn publish_mr_review(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<PublishResponse>, ApiError> {
    let response = mr_reviews::publish(&state.pool, &state.config, id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn ignore_mr_review(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    mr_reviews::ignore(&state.pool, id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AgentTurnRequest {
    message: String,
}

async fn agent_turn_mr_review(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<AgentTurnRequest>,
) -> Result<Json<AgentTurnResponse>, ApiError> {
    let message = body.message.trim();
    if message.is_empty() {
        return Err(ApiError::from(Error::InvalidProjectConfig(
            "message is required".into(),
        )));
    }
    let response = mr_reviews::agent_turn(&state.pool, &state.config, id, message)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(response))
}

async fn mr_scan(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
    Query(query): Query<MrScanQuery>,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    let force = runs::parse_mr_scan_force(query.force.as_deref());
    let run_id = runs::create_manual_mr_scan_run(&state.pool, project_id, force).await?;

    if let Some(worker) = &state.worker {
        worker.wake();
    }

    Ok((StatusCode::ACCEPTED, Json(CreateRunResponse { run_id })))
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
            Error::RunConflict
            | Error::DuplicateDisplayName
            | Error::PeopleDirectoryConflict
            | Error::DuplicateProjectName
            | Error::IdentityConflict
            | Error::MrReviewConflict
            | Error::PendingItemAlreadyResolved => StatusCode::CONFLICT,
            Error::UnsupportedRunTrigger(_)
            | Error::InvalidPersonName
            | Error::InvalidIdentityValue
            | Error::InvalidProjectName
            | Error::InvalidProjectConfig(_)
            | Error::InvalidPendingItemStatus
            | Error::InvalidPendingItemListStatus => StatusCode::BAD_REQUEST,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::AgentFailed(_) | Error::NotesSyncFailed(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.0.to_string()).into_response()
    }
}
