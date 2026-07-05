use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use crate::error::{Error, Result};

pub async fn init_pool(data_dir: &Path) -> Result<SqlitePool> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::create_dir_all(data_dir.join("repos"))?;
    std::fs::create_dir_all(data_dir.join("reports"))?;

    let db_path = data_dir.join("reviewer.db");
    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .map_err(Error::Database)?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .map_err(Error::Database)?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(Error::Migrate)?;

    Ok(pool)
}

pub async fn foreign_keys_enabled(pool: &SqlitePool) -> Result<bool> {
    let row = sqlx::query("PRAGMA foreign_keys")
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    let enabled: i64 = row.get(0);
    Ok(enabled == 1)
}

pub async fn table_exists(pool: &SqlitePool, table: &str) -> Result<bool> {
    let row = sqlx::query(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;
    let count: i64 = row.get(0);
    Ok(count > 0)
}
