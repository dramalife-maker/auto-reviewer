use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing required environment variable DATA_ROOT_DIR")]
    MissingDataDir,
    #[error("invalid PORT value: {0}")]
    InvalidPort(String),
    #[error("projects config not found: {0}")]
    ProjectsConfigNotFound(String),
    #[error("a project is already queued or running")]
    RunConflict,
    #[error("unsupported run trigger: {0}")]
    UnsupportedRunTrigger(String),
    #[error("failed to parse summary.md: {0}")]
    SummaryParse(String),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
}
