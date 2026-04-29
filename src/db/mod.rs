use std::time::Duration;

use anyhow::Context;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// Database connection pool type alias.
///
/// To migrate to PostgreSQL:
///   1. Change this alias to `pub type DbPool = sqlx::PgPool;`
///   2. Replace `SqlitePoolOptions` with `PgPoolOptions` in `init()`
///   3. Remove SQLite PRAGMAs from the `after_connect` block
///   4. Replace all `?` bind placeholders with `$1`, `$2`, … in raw SQL strings
///   5. Replace `lower(hex(randomblob(16)))` with `gen_random_uuid()` in migrations
///   6. Replace `datetime('now')` / `CURRENT_TIMESTAMP` TEXT with `NOW()` timestamps
///   7. Replace `INTEGER` boolean columns with `BOOLEAN`; `TEXT` JSON with `JSONB`
///   8. Delete `db/uuid_sql.rs` — PostgreSQL supports native UUID equality (`= $1`)
pub type DbPool = SqlitePool;

pub async fn init() -> anyhow::Result<DbPool> {
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
