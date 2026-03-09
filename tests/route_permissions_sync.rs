use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::{Row, SqlitePool};

const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch", "head", "options"];

struct TempPool {
    pool: SqlitePool,
    _tmp_dir: tempfile::TempDir,
}

fn canonical_pattern(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            out.push(':');
            while let Some(inner) = chars.next() {
                if inner == '}' {
                    break;
                }
                out.push(inner);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn protected_openapi_operations(spec: &Value) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();

    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return out;
    };

    for (path, path_item) in paths {
        // Public endpoints are intentionally excluded from route_permissions.
        if path.starts_with("/auth") || path == "/api/health" {
            continue;
        }

        let Some(path_obj) = path_item.as_object() else {
            continue;
        };
        let pattern = canonical_pattern(path);
        for (method, _) in path_obj {
            if HTTP_METHODS.contains(&method.as_str()) {
                out.insert((method.to_uppercase(), pattern.clone()));
            }
        }
    }

    out
}

async fn migrated_temp_db() -> anyhow::Result<TempPool> {
    let tmp_dir = tempfile::tempdir()?;
    let db_path: PathBuf = tmp_dir.path().join("route-perm-check.sqlite");

    let opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;

    sqlx::migrate!().run(&pool).await?;
    Ok(TempPool {
        pool,
        _tmp_dir: tmp_dir,
    })
}

#[tokio::test]
async fn route_permissions_match_protected_openapi_operations() -> anyhow::Result<()> {
    let temp = migrated_temp_db().await?;
    let pool = &temp.pool;
    let openapi = serde_json::to_value(s_curve::docs::build_openapi(8000)?)?;
    let expected = protected_openapi_operations(&openapi);

    let rows = sqlx::query("SELECT UPPER(method) AS method, route_pattern FROM route_permissions")
        .fetch_all(pool)
        .await?;
    let actual: BTreeSet<(String, String)> = rows
        .iter()
        .map(|r: &SqliteRow| {
            (
                r.get::<String, _>("method"),
                r.get::<String, _>("route_pattern"),
            )
        })
        .collect();

    let missing: Vec<_> = expected.difference(&actual).cloned().collect();
    let extra: Vec<_> = actual.difference(&expected).cloned().collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "route_permissions mismatch. Missing: {:?}. Extra: {:?}",
        missing,
        extra
    );

    Ok(())
}

#[tokio::test]
async fn route_permissions_schema_has_updated_at_column() -> anyhow::Result<()> {
    let temp = migrated_temp_db().await?;
    let pool = &temp.pool;

    let rows = sqlx::query("PRAGMA table_info(route_permissions)")
        .fetch_all(pool)
        .await?;

    let columns: Vec<String> = rows.iter().map(|r: &SqliteRow| r.get("name")).collect();

    assert!(
        columns.iter().any(|c| c == "updated_at"),
        "route_permissions.updated_at column is missing"
    );

    Ok(())
}
