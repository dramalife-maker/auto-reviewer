use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const DATA_ROOT_DIR_ENV: &str = "DATA_ROOT_DIR";
const PORT_ENV: &str = "PORT";
const PROJECTS_CONFIG_ENV: &str = "PROJECTS_CONFIG";
const APP_ROOT_ENV: &str = "APP_ROOT";
const CORS_ALLOW_ORIGINS_ENV: &str = "CORS_ALLOW_ORIGINS";
const REVIEWER_AGENT_ENV: &str = "REVIEWER_AGENT";
const REVIEWER_MODEL_ENV: &str = "REVIEWER_MODEL";
const REVIEWER_EXECUTOR_ENV: &str = "REVIEWER_EXECUTOR";
const CORS_ALLOW_ANY: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewerAgent {
    Claude,
    Cursor,
}

impl ReviewerAgent {
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewerAgent::Claude => "claude",
            ReviewerAgent::Cursor => "cursor",
        }
    }

    pub fn parse_db_value(raw: &str) -> Self {
        if raw.eq_ignore_ascii_case("claude") {
            ReviewerAgent::Claude
        } else {
            ReviewerAgent::Cursor
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub port: u16,
    pub projects_config_path: PathBuf,
    pub app_root: PathBuf,
    pub cors_allow_origins: Vec<String>,
    pub reviewer_agent: ReviewerAgent,
    pub reviewer_model: Option<String>,
    pub reviewer_executor: Option<PathBuf>,
}

impl AppConfig {
    fn with_defaults() -> Self {
        Self {
            data_dir: PathBuf::new(),
            port: 8080,
            projects_config_path: PathBuf::from("projects.yaml"),
            app_root: PathBuf::new(),
            cors_allow_origins: Vec::new(),
            reviewer_agent: ReviewerAgent::Cursor,
            reviewer_model: None,
            reviewer_executor: None,
        }
    }

    fn port_from_env() -> Result<u16> {
        let default_port = Self::with_defaults().port;
        match std::env::var(PORT_ENV) {
            Ok(value) => {
                let port: u16 = value
                    .parse()
                    .map_err(|_| Error::InvalidPort(value.clone()))?;
                if port == 0 {
                    return Err(Error::InvalidPort(value));
                }
                Ok(port)
            }
            Err(_) => Ok(default_port),
        }
    }

    fn projects_config_path_from_env() -> PathBuf {
        std::env::var(PROJECTS_CONFIG_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::with_defaults().projects_config_path)
    }

    fn app_root_from_env() -> PathBuf {
        std::env::var(APP_ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
    }

    fn cors_allow_origins_from_env() -> Result<Vec<String>> {
        let raw = std::env::var(CORS_ALLOW_ORIGINS_ENV).unwrap_or_default();
        let origins: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect();

        if origins.is_empty() {
            #[cfg(debug_assertions)]
            {
                return Ok(vec![CORS_ALLOW_ANY.to_string()]);
            }
            #[cfg(not(debug_assertions))]
            {
                return Ok(Vec::new());
            }
        }

        if origins.iter().any(|origin| origin == CORS_ALLOW_ANY) {
            if origins.len() > 1 {
                return Err(Error::InvalidCorsOrigin(CORS_ALLOW_ANY.to_string()));
            }
            return Ok(origins);
        }

        for origin in &origins {
            if !origin.starts_with("http://") && !origin.starts_with("https://") {
                return Err(Error::InvalidCorsOrigin(origin.clone()));
            }
            origin
                .parse::<http::HeaderValue>()
                .map_err(|_| Error::InvalidCorsOrigin(origin.clone()))?;
        }

        Ok(origins)
    }

    fn reviewer_agent_from_env() -> ReviewerAgent {
        match std::env::var(REVIEWER_AGENT_ENV) {
            Ok(value) if value.eq_ignore_ascii_case("claude") => ReviewerAgent::Claude,
            _ => ReviewerAgent::Cursor,
        }
    }

    fn reviewer_model_from_env() -> Option<String> {
        std::env::var(REVIEWER_MODEL_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn reviewer_executor_from_env() -> Option<PathBuf> {
        std::env::var(REVIEWER_EXECUTOR_ENV)
            .ok()
            .map(PathBuf::from)
    }

    pub fn from_env() -> Result<Self> {
        let data_dir = std::env::var(DATA_ROOT_DIR_ENV).map_err(|_| Error::MissingDataDir)?;
        Ok(Self {
            data_dir: PathBuf::from(data_dir),
            port: Self::port_from_env()?,
            projects_config_path: Self::projects_config_path_from_env(),
            app_root: Self::app_root_from_env(),
            cors_allow_origins: Self::cors_allow_origins_from_env()?,
            reviewer_agent: Self::reviewer_agent_from_env(),
            reviewer_model: Self::reviewer_model_from_env(),
            reviewer_executor: Self::reviewer_executor_from_env(),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn listen_addr(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port))
    }

    pub fn projects_config_path(&self) -> &Path {
        &self.projects_config_path
    }

    pub fn app_root(&self) -> &Path {
        &self.app_root
    }

    pub fn cors_allow_origins(&self) -> &[String] {
        &self.cors_allow_origins
    }

    pub fn reviewer_agent(&self) -> ReviewerAgent {
        self.reviewer_agent
    }

    pub fn reviewer_model(&self) -> Option<&str> {
        self.reviewer_model.as_deref()
    }

    pub fn reviewer_executor(&self) -> Option<&Path> {
        self.reviewer_executor.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn restore_port_env(previous: Option<String>) {
        match previous {
            Some(value) => std::env::set_var(PORT_ENV, value),
            None => std::env::remove_var(PORT_ENV),
        }
    }

    #[test]
    fn port_defaults_when_unset() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(PORT_ENV).ok();
        std::env::remove_var(PORT_ENV);

        assert_eq!(
            AppConfig::port_from_env().expect("default port"),
            AppConfig::with_defaults().port
        );

        restore_port_env(previous);
    }

    #[test]
    fn port_reads_from_env() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(PORT_ENV).ok();
        std::env::set_var(PORT_ENV, "9090");

        assert_eq!(AppConfig::port_from_env().expect("port from env"), 9090);

        restore_port_env(previous);
    }

    #[test]
    fn port_rejects_invalid_value() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(PORT_ENV).ok();
        std::env::set_var(PORT_ENV, "not-a-port");

        assert!(matches!(
            AppConfig::port_from_env(),
            Err(Error::InvalidPort(_))
        ));

        restore_port_env(previous);
    }

    fn restore_cors_env(previous: Option<String>) {
        match previous {
            Some(value) => std::env::set_var(CORS_ALLOW_ORIGINS_ENV, value),
            None => std::env::remove_var(CORS_ALLOW_ORIGINS_ENV),
        }
    }

    #[test]
    fn cors_origins_default_when_unset() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(CORS_ALLOW_ORIGINS_ENV).ok();
        std::env::remove_var(CORS_ALLOW_ORIGINS_ENV);

        let origins = AppConfig::cors_allow_origins_from_env().expect("default cors");
        #[cfg(debug_assertions)]
        assert_eq!(origins, vec![CORS_ALLOW_ANY.to_string()]);
        #[cfg(not(debug_assertions))]
        assert!(origins.is_empty());

        restore_cors_env(previous);
    }

    #[test]
    fn cors_origins_accepts_wildcard() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(CORS_ALLOW_ORIGINS_ENV).ok();
        std::env::set_var(CORS_ALLOW_ORIGINS_ENV, CORS_ALLOW_ANY);

        assert_eq!(
            AppConfig::cors_allow_origins_from_env().expect("wildcard cors"),
            vec![CORS_ALLOW_ANY.to_string()]
        );

        restore_cors_env(previous);
    }

    #[test]
    fn cors_origins_reject_wildcard_with_other_origins() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(CORS_ALLOW_ORIGINS_ENV).ok();
        std::env::set_var(
            CORS_ALLOW_ORIGINS_ENV,
            "*,https://reviewer.example.com",
        );

        assert!(matches!(
            AppConfig::cors_allow_origins_from_env(),
            Err(Error::InvalidCorsOrigin(_))
        ));

        restore_cors_env(previous);
    }

    #[test]
    fn cors_origins_parse_comma_separated() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(CORS_ALLOW_ORIGINS_ENV).ok();
        std::env::set_var(
            CORS_ALLOW_ORIGINS_ENV,
            "https://reviewer.example.com, http://localhost:5173",
        );

        assert_eq!(
            AppConfig::cors_allow_origins_from_env().expect("cors origins"),
            vec![
                "https://reviewer.example.com".to_string(),
                "http://localhost:5173".to_string(),
            ]
        );

        restore_cors_env(previous);
    }

    #[test]
    fn cors_origins_reject_invalid_value() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(CORS_ALLOW_ORIGINS_ENV).ok();
        std::env::set_var(CORS_ALLOW_ORIGINS_ENV, "not-a-url");

        assert!(matches!(
            AppConfig::cors_allow_origins_from_env(),
            Err(Error::InvalidCorsOrigin(_))
        ));

        restore_cors_env(previous);
    }

    fn restore_reviewer_agent_env(previous: Option<String>) {
        match previous {
            Some(value) => std::env::set_var(REVIEWER_AGENT_ENV, value),
            None => std::env::remove_var(REVIEWER_AGENT_ENV),
        }
    }

    #[test]
    fn reviewer_agent_defaults_to_cursor() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(REVIEWER_AGENT_ENV).ok();
        std::env::remove_var(REVIEWER_AGENT_ENV);

        assert_eq!(
            AppConfig::reviewer_agent_from_env(),
            ReviewerAgent::Cursor
        );

        restore_reviewer_agent_env(previous);
    }

    #[test]
    fn reviewer_agent_reads_claude() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(REVIEWER_AGENT_ENV).ok();
        std::env::set_var(REVIEWER_AGENT_ENV, "claude");

        assert_eq!(
            AppConfig::reviewer_agent_from_env(),
            ReviewerAgent::Claude
        );

        restore_reviewer_agent_env(previous);
    }

    #[test]
    fn reviewer_agent_reads_cursor() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var(REVIEWER_AGENT_ENV).ok();
        std::env::set_var(REVIEWER_AGENT_ENV, "cursor");

        assert_eq!(
            AppConfig::reviewer_agent_from_env(),
            ReviewerAgent::Cursor
        );

        restore_reviewer_agent_env(previous);
    }
}
