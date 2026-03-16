use anyhow::{Context, Result};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const MUTABLE_TABLES: &[&str] = &[
    "activity_log",
    "event_store",
    "password_reset_tokens",
    "stale_important_logs",
    "stale_noise_logs",
    "telemetry_events",
    "task_progress",
    "task_progress_components",
    "task_dependencies",
    "tasks",
    "task_health_rules",
    "task_health_rule_sets",
    "project_plan",
    "project_members",
    "project_member_resource_roles",
    "project_resource_role_rates",
    "work_logs",
    "s_curve_stage_rules",
    "s_curve_stage_rule_sets",
    "projects",
    "role_permissions",
    "user_permissions",
    "user_roles",
    "users",
];

pub struct TestDb {
    pub pool: SqlitePool,
    #[allow(dead_code)]
    pub db_path: PathBuf,
    _tmp_dir: TempDir,
}

pub async fn cloned_clean_db() -> Result<TestDb> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("scurve.sqlite");
    if !source.exists() {
        anyhow::bail!("template database is missing: {}", source.display());
    }

    let tmp_dir = tempfile::tempdir().context("failed to create temp dir for test db")?;
    let db_path = tmp_dir.path().join("scurve.sqlite");

    fs::copy(&source, &db_path).with_context(|| {
        format!(
            "failed to clone template database from {} to {}",
            source.display(),
            db_path.display()
        )
    })?;

    let opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA busy_timeout = 5000")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA journal_mode = WAL")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA synchronous = NORMAL")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect_with(opts)
        .await?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("failed to run migrations on cloned test database")?;

    reset_mutable_tables(&pool).await?;

    Ok(TestDb {
        pool,
        db_path,
        _tmp_dir: tmp_dir,
    })
}

async fn reset_mutable_tables(pool: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(pool)
        .await?;

    for table in MUTABLE_TABLES {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
                .bind(table)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            let stmt = format!("DELETE FROM {table}");
            sqlx::query(&stmt).execute(pool).await?;
        }
    }

    // Keep FK enforcement disabled to match existing integration test behavior.
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(pool)
        .await?;
    Ok(())
}
