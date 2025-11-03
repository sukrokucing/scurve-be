use sqlx::Row;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use chrono::Utc;
use dotenvy::dotenv;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

#[derive(Parser, Debug)]
#[command(author, version, about = "s-curve migration tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a new empty migration with the provided name
    MakeMigration { name: String },
    /// Apply pending migrations
    MigrateRun,
    /// Show migration status against the current database
    MigrateStatus,
    /// Roll back the last applied migration
    MigrateRollback,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Try to load env from CWD; when running in Docker the binary CWD may differ,
    // so fall back to the crate-local `.env` using CARGO_MANIFEST_DIR.
    if dotenv().is_err() {
        let crate_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
        let _ = dotenvy::from_path(crate_env);
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::MakeMigration { name } => {
            let path = make_migration_file(&name)?;
            println!("Created migration: {}", path.display());
        }
        Commands::MigrateRun => {
            let pool = get_pool().await?;
            let migrator = get_migrator().await?;
            migrator.run(&pool).await?;
            println!("Migrations applied");
        }
        Commands::MigrateStatus => {
            let pool = get_pool().await?;
            let migrator = get_migrator().await?;
            print_status(&pool, &migrator).await?;
        }
        Commands::MigrateRollback => {
            let pool = get_pool().await?;
            let migrator = get_migrator().await?;
            migrator
                .undo(&pool, 1)
                .await
                .context("no migrations were rolled back")?;
            println!("Rolled back last migration");
        }
    }

    Ok(())
}

fn make_migration_file(name: &str) -> anyhow::Result<PathBuf> {
    let timestamp = Utc::now().format("%Y_%m_%d_%H%M%S");
    let sanitized = sanitize_name(name);
    let filename = format!("{}_{}.sql", timestamp, sanitized);
    let path = Path::new("migrations").join(filename);

    if path.exists() {
        anyhow::bail!("migration already exists: {}", path.display());
    }

    fs::write(&path, "-- Write your migration SQL here\n")
        .with_context(|| format!("failed to create migration at {}", path.display()))?;

    Ok(path)
}

async fn get_pool() -> anyhow::Result<SqlitePool> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("failed to connect to database")
}

async fn print_status(pool: &SqlitePool, migrator: &sqlx::migrate::Migrator) -> anyhow::Result<()> {
    // use sqlx::migrate::MigrationType;
    // use std::collections::HashMap;

    // If the migrations table doesn't exist, nothing is applied yet
    let db_applied = sqlx::query!("SELECT name FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'")
        .fetch_optional(pool)
        .await?;
    let applied_versions: HashSet<i64> = if db_applied.is_some() {
        let rows = sqlx::query("SELECT version FROM _sqlx_migrations WHERE success = 1")
            .fetch_all(pool)
            .await?;
        rows.iter().filter_map(|row| row.try_get::<i64, _>("version").ok()).collect()
    } else {
        HashSet::new()
    };

    println!("{:<8} {:<20} {}", "Status", "Version", "Name");
    for migration in migrator.iter() {
        let version = migration.version;
        let applied = applied_versions.contains(&version);
        let status = if applied { "applied" } else { "pending" };
        let desc = migration.description.as_ref().trim();
        let name = if !desc.is_empty() {
            desc
        } else {
            "unknown"
        };
        println!("{:<8} {:<20} {}", status, version, name);
    }

    Ok(())
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '_' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}

async fn get_migrator() -> anyhow::Result<sqlx::migrate::Migrator> {
    // Try local ./migrations first (when running from repo root). If that
    // doesn't exist (common in containers where CWD differs), fall back to
    // the crate-local migrations folder determined by CARGO_MANIFEST_DIR.
    let local = Path::new("./migrations");
    let migrator_path = if local.exists() {
        local.to_path_buf()
    } else {
        let crate_mig = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        crate_mig
    };

    let migrator_path_display = migrator_path.display().to_string();
    sqlx::migrate::Migrator::new(migrator_path)
        .await
        .with_context(|| format!("failed to load migrations from {}", migrator_path_display))
}
