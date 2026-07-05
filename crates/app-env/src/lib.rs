//! Load `.env` files for local development.
//!
//! Variables already present in the process environment are never overwritten.
//! Typical usage at the start of `main`:
//!
//! ```no_run
//! app_env::load();
//! ```

use std::path::{Path, PathBuf};

const DEFAULT_FALLBACKS: &[&str] = &["../.env"];

/// Load `.env` from the current working directory, then default fallbacks.
///
/// Returns the path of the file that was loaded, if any.
pub fn load() -> Option<PathBuf> {
    EnvLoader::new().load()
}

/// Load `.env` from the current working directory, then the given fallback paths.
pub fn load_with_fallbacks(fallbacks: &[&str]) -> Option<PathBuf> {
    EnvLoader::new()
        .with_fallbacks(fallbacks.iter().copied())
        .load()
}

/// Configurable `.env` loader for projects with custom search paths.
#[derive(Clone, Debug, Default)]
pub struct EnvLoader {
    fallbacks: Vec<PathBuf>,
}

impl EnvLoader {
    pub fn new() -> Self {
        Self {
            fallbacks: DEFAULT_FALLBACKS.iter().map(PathBuf::from).collect(),
        }
    }

    pub fn with_fallback(mut self, path: impl Into<PathBuf>) -> Self {
        self.fallbacks.push(path.into());
        self
    }

    pub fn with_fallbacks(
        mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Self {
        self.fallbacks
            .extend(paths.into_iter().map(|path| path.as_ref().to_path_buf()));
        self
    }

    /// Try `./.env` first, then each fallback path in order.
    pub fn load(&self) -> Option<PathBuf> {
        if let Ok(path) = dotenvy::dotenv() {
            return Some(path);
        }

        for path in &self.fallbacks {
            if dotenvy::from_path(path).is_ok() {
                return Some(path.clone());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn loads_variables_from_explicit_path() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var("APP_ENV_TEST_KEY").ok();

        let temp = tempfile::tempdir().expect("tempdir");
        let env_file = temp.path().join(".env");
        std::fs::write(&env_file, "APP_ENV_TEST_KEY=from_file\n").expect("write env");

        std::env::remove_var("APP_ENV_TEST_KEY");
        assert!(dotenvy::from_path(&env_file).is_ok());
        assert_eq!(
            std::env::var("APP_ENV_TEST_KEY").expect("var"),
            "from_file"
        );

        restore_var("APP_ENV_TEST_KEY", previous.as_deref());
    }

    #[test]
    fn does_not_override_existing_environment() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var("APP_ENV_TEST_KEY").ok();

        let temp = tempfile::tempdir().expect("tempdir");
        let env_file = temp.path().join(".env");
        std::fs::write(&env_file, "APP_ENV_TEST_KEY=from_file\n").expect("write env");

        std::env::set_var("APP_ENV_TEST_KEY", "from_shell");
        let _ = dotenvy::from_path(&env_file);
        assert_eq!(
            std::env::var("APP_ENV_TEST_KEY").expect("var"),
            "from_shell"
        );

        restore_var("APP_ENV_TEST_KEY", previous.as_deref());
    }

    #[test]
    fn loader_reads_fallback_path() {
        let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
        let previous = std::env::var("APP_ENV_TEST_KEY").ok();

        let temp = tempfile::tempdir().expect("tempdir");
        let fallback = temp.path().join("nested.env");
        std::fs::write(&fallback, "APP_ENV_TEST_KEY=from_fallback\n").expect("write env");

        std::env::remove_var("APP_ENV_TEST_KEY");

        let loaded = EnvLoader::new()
            .with_fallback(fallback.clone())
            .load()
            .expect("loaded");

        assert_eq!(loaded, fallback);
        assert_eq!(
            std::env::var("APP_ENV_TEST_KEY").expect("var"),
            "from_fallback"
        );

        restore_var("APP_ENV_TEST_KEY", previous.as_deref());
    }

    fn restore_var(key: &str, previous: Option<&str>) {
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
