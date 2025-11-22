use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::Utc;

use s_curve::db::row_parsers::{db_progress_from_row, db_task_from_row, db_project_from_row, db_user_from_row, db_project_plan_point_from_row};

async fn setup_pool() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:").await.expect("connect")
}

#[tokio::test]
async fn parse_progress_row_text_uuid() {
    let pool = setup_pool().await;
    sqlx::query(
        "CREATE TABLE task_progress (id TEXT, project_id TEXT, task_id TEXT, progress INTEGER, note TEXT, created_at TEXT, updated_at TEXT, deleted_at TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let created = Utc::now().to_rfc3339();
    let updated = Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO task_progress (id, project_id, task_id, progress, note, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(task_id.to_string())
        .bind(42i32)
        .bind(Some("note".to_string()))
        .bind(created.clone())
        .bind(updated.clone())
        .bind(Option::<String>::None)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT * FROM task_progress WHERE id = ?")
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

    let parsed = db_progress_from_row(&row).expect("parse");
    assert_eq!(parsed.id, id);
    assert_eq!(parsed.project_id, project_id);
    assert_eq!(parsed.task_id, task_id);
    assert_eq!(parsed.progress, 42);
    assert_eq!(parsed.note.unwrap(), "note");
}

#[tokio::test]
async fn parse_task_row_text_uuid() {
    let pool = setup_pool().await;
    sqlx::query(
        "CREATE TABLE tasks (id TEXT, project_id TEXT, title TEXT, status TEXT, due_date TEXT, start_date TEXT, end_date TEXT, duration_days INTEGER, assignee TEXT, parent_id TEXT, progress INTEGER, created_at TEXT, updated_at TEXT, deleted_at TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let assignee = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, due_date, start_date, end_date, duration_days, assignee, parent_id, progress, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind("T")
        .bind("pending")
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<i32>::None)
        .bind(Some(assignee.to_string()))
        .bind(Option::<String>::None)
        .bind(0i32)
        .bind(now.clone())
        .bind(now.clone())
        .bind(Option::<String>::None)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

    let parsed = db_task_from_row(&row).expect("parse");
    assert_eq!(parsed.id, id);
    assert_eq!(parsed.project_id, project_id);
    assert_eq!(parsed.title, "T");
    assert_eq!(parsed.assignee.unwrap(), assignee);
}

#[tokio::test]
async fn parse_project_row_text_uuid() {
    let pool = setup_pool().await;
    sqlx::query(
        "CREATE TABLE projects (id TEXT, user_id TEXT, name TEXT, description TEXT, theme_color TEXT, created_at TEXT, updated_at TEXT, deleted_at TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(id.to_string())
        .bind(user_id.to_string())
        .bind("P")
        .bind(Option::<String>::None)
        .bind("#fff")
        .bind(now.clone())
        .bind(now.clone())
        .bind(Option::<String>::None)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT * FROM projects WHERE id = ?")
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

    let parsed = db_project_from_row(&row).expect("parse");
    assert_eq!(parsed.id, id);
    assert_eq!(parsed.user_id, user_id);
    assert_eq!(parsed.name, "P");
}

#[tokio::test]
async fn parse_user_row_text_uuid() {
    let pool = setup_pool().await;
    sqlx::query(
        "CREATE TABLE users (id TEXT, name TEXT, email TEXT, password_hash TEXT, provider TEXT, provider_id TEXT, created_at TEXT, updated_at TEXT, deleted_at TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, provider_id, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(id.to_string())
        .bind("U")
        .bind("u@example.com")
        .bind("hash")
        .bind("local")
        .bind(Option::<String>::None)
        .bind(now.clone())
        .bind(now.clone())
        .bind(Option::<String>::None)
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT * FROM users WHERE id = ?")
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

    let parsed = db_user_from_row(&row).expect("parse");
    assert_eq!(parsed.id, id);
    assert_eq!(parsed.name, "U");
    assert_eq!(parsed.email, "u@example.com");
}

#[tokio::test]
async fn parse_project_plan_point_row_text_uuid() {
    let pool = setup_pool().await;
    sqlx::query(
        "CREATE TABLE project_plan (id TEXT, project_id TEXT, date TEXT, planned_progress INTEGER, created_at TEXT, updated_at TEXT)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let date = Utc::now().to_rfc3339();
    let now = Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO project_plan (id, project_id, date, planned_progress, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(date.clone())
        .bind(10i32)
        .bind(now.clone())
        .bind(now.clone())
        .execute(&pool)
        .await
        .unwrap();

    let row = sqlx::query("SELECT * FROM project_plan WHERE id = ?")
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

    let parsed = db_project_plan_point_from_row(&row).expect("parse");
    assert_eq!(parsed.id, id);
    assert_eq!(parsed.project_id, project_id);
    assert_eq!(parsed.planned_progress, 10i32);
}
