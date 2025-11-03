use std::time::Duration;

use anyhow::Context;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub async fn init() -> anyhow::Result<SqlitePool> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database_url)
        .await
        .context("failed to connect to database")?;

    Ok(pool)
}
