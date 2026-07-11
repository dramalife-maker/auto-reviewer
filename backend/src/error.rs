use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing required environment variable DATA_ROOT_DIR")]
    MissingDataDir,
    #[error("invalid PORT value: {0}")]
    InvalidPort(String),
    #[error("invalid CORS_ALLOW_ORIGINS entry: {0}")]
    InvalidCorsOrigin(String),
    #[error("projects config not found: {0}")]
    ProjectsConfigNotFound(String),
    #[error("a project is already queued or running")]
    RunConflict,
    #[error("unsupported run trigger: {0}")]
    UnsupportedRunTrigger(String),
    #[error("not found")]
    NotFound,
    #[error("person display_name already exists")]
    DuplicateDisplayName,
    #[error("identity is already bound to another person")]
    IdentityConflict,
    #[error("invalid person display_name")]
    InvalidPersonName,
    #[error("invalid identity value")]
    InvalidIdentityValue,
    #[error("project name already exists")]
    DuplicateProjectName,
    #[error("invalid project name")]
    InvalidProjectName,
    #[error("invalid project configuration: {0}")]
    InvalidProjectConfig(String),
    #[error("failed to parse summary.md: {0}")]
    SummaryParse(String),
    #[error("mr review conflict")]
    MrReviewConflict,
    #[error("agent failed: {0}")]
    AgentFailed(String),
    #[error("pending item status must be 'resolved'")]
    InvalidPendingItemStatus,
    #[error("pending item is already resolved")]
    PendingItemAlreadyResolved,
    #[error("invalid pending item list status filter; use open, resolved, or all")]
    InvalidPendingItemListStatus,
    #[error("failed to sync notes file: {0}")]
    NotesSyncFailed(String),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
}
