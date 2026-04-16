use std::time::Duration;

use anyhow::Context;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub async fn init() -> anyhow::Result<SqlitePool> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;

    // SQLite WAL serialises writers: a pool larger than ~5 connections wastes memory
    // without improving throughput. Concurrent readers are handled fine with 5.
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA busy_timeout = 5000")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA journal_mode = WAL")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA synchronous = NORMAL")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA temp_store = MEMORY")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA cache_size = -20000")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await
        .context("failed to connect to database")?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    Ok(pool)
}

pub mod row_parsers;
pub mod uuid_sql;
