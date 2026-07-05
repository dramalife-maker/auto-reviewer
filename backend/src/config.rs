use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const DATA_ROOT_DIR_ENV: &str = "DATA_ROOT_DIR";
const PORT_ENV: &str = "PORT";
const PROJECTS_CONFIG_ENV: &str = "PROJECTS_CONFIG";
const APP_ROOT_ENV: &str = "APP_ROOT";

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub port: u16,
    pub projects_config_path: PathBuf,
    pub app_root: PathBuf,
}

impl AppConfig {
    fn with_defaults() -> Self {
        Self {
            data_dir: PathBuf::new(),
            port: 8080,
            projects_config_path: PathBuf::from("projects.yaml"),
            app_root: PathBuf::new(),
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

    pub fn from_env() -> Result<Self> {
        let data_dir = std::env::var(DATA_ROOT_DIR_ENV).map_err(|_| Error::MissingDataDir)?;
        Ok(Self {
            data_dir: PathBuf::from(data_dir),
            port: Self::port_from_env()?,
            projects_config_path: Self::projects_config_path_from_env(),
            app_root: Self::app_root_from_env(),
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
}
