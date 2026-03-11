#![allow(clippy::uninlined_format_args)]

mod support;

use uuid::Uuid;

#[tokio::test]
async fn test_cycle_detection_returns_error() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Data: create a simple 3-node cycle A->B, B->C, C->A
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string()).bind(user_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'A', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(a.to_string()).bind(project_id.to_string()).bind(1i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'B', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(b.to_string()).bind(project_id.to_string()).bind(1i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'C', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(c.to_string()).bind(project_id.to_string()).bind(1i64).execute(&pool).await?;

    // dependencies forming cycle
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(a.to_string()).bind(b.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(b.to_string()).bind(c.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(c.to_string()).bind(a.to_string()).execute(&pool).await?;

    // Call endpoint and expect an error
    use axum::extract::{Path as AxPath, State as AxState};
    use s_curve::app::AppState;
    use s_curve::jwt::{AuthUser, JwtConfig};
    use s_curve::routes::projects::get_project_critical_path;

    let jwt = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret".to_vec()),
        exp_hours: 24,
    };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(
        pool.clone(),
        jwt,
        event_bus,
        s_curve::authz::RoutePermissionCache::new(),
    );
    let auth = AuthUser { user_id };

    let path = AxPath(project_id);
    let res = get_project_critical_path(AxState(app_state.clone()), auth.clone(), path).await;
    assert!(res.is_err(), "expected error for cyclic dependency graph");

    Ok(())
}

#[tokio::test]
async fn test_disconnected_graph_picks_longest_component() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Data: two components. Comp1: A->B (total 5). Comp2: C->D->E (total 9)
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();
    let e = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string()).bind(user_id.to_string()).execute(&pool).await?;

    // comp1
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'A', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(a.to_string()).bind(project_id.to_string()).bind(2i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'B', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(b.to_string()).bind(project_id.to_string()).bind(3i64).execute(&pool).await?;
    // comp2
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'C', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(c.to_string()).bind(project_id.to_string()).bind(1i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'D', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(d.to_string()).bind(project_id.to_string()).bind(4i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'E', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(e.to_string()).bind(project_id.to_string()).bind(4i64).execute(&pool).await?;

    // deps
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(a.to_string()).bind(b.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(c.to_string()).bind(d.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(d.to_string()).bind(e.to_string()).execute(&pool).await?;

    use axum::extract::{Path as AxPath, State as AxState};
    use s_curve::app::AppState;
    use s_curve::jwt::{AuthUser, JwtConfig};
    use s_curve::routes::projects::get_project_critical_path;

    let jwt = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret".to_vec()),
        exp_hours: 24,
    };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(
        pool.clone(),
        jwt,
        event_bus,
        s_curve::authz::RoutePermissionCache::new(),
    );
    let auth = AuthUser { user_id };

    // call endpoint
    let path = AxPath(project_id);
    let res = get_project_critical_path(AxState(app_state.clone()), auth.clone(), path).await?;
    let ids = res.0.task_ids;

    // Expect component C->D->E to be chosen
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], c);
    assert_eq!(ids[1], d);
    assert_eq!(ids[2], e);

    Ok(())
}

#[tokio::test]
async fn test_equal_length_paths_returns_valid_path_of_expected_length() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Data: two paths A->B->C and X->Y with equal total duration
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let x = Uuid::new_v4();
    let y = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string()).bind(user_id.to_string()).execute(&pool).await?;

    // A->B->C durations: 2 + 2 + 2 = 6
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'A', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(a.to_string()).bind(project_id.to_string()).bind(2i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'B', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(b.to_string()).bind(project_id.to_string()).bind(2i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'C', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(c.to_string()).bind(project_id.to_string()).bind(2i64).execute(&pool).await?;

    // X->Y durations: 3 + 3 = 6
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'X', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(x.to_string()).bind(project_id.to_string()).bind(3i64).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, duration_days, created_at, updated_at) VALUES (?, ?, 'Y', 'todo', ?, datetime('now'), datetime('now'))")
        .bind(y.to_string()).bind(project_id.to_string()).bind(3i64).execute(&pool).await?;

    // deps A->B, B->C and X->Y
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(a.to_string()).bind(b.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(b.to_string()).bind(c.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(x.to_string()).bind(y.to_string()).execute(&pool).await?;

    use axum::extract::{Path as AxPath, State as AxState};
    use s_curve::app::AppState;
    use s_curve::jwt::{AuthUser, JwtConfig};
    use s_curve::routes::projects::get_project_critical_path;

    let jwt = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret".to_vec()),
        exp_hours: 24,
    };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(
        pool.clone(),
        jwt,
        event_bus,
        s_curve::authz::RoutePermissionCache::new(),
    );
    let auth = AuthUser { user_id };

    let path = AxPath(project_id);
    let res = get_project_critical_path(AxState(app_state.clone()), auth.clone(), path).await?;
    let ids = res.0.task_ids;

    // The returned path should have total duration 6 and be one of the two valid paths.
    // We assert the length and that nodes form a valid chained path.
    let mut total_duration: i64 = 0;
    for id in ids.iter() {
        let dur: i64 =
            sqlx::query_scalar("SELECT COALESCE(duration_days, 0) FROM tasks WHERE id = ?")
                .bind(id.to_string())
                .fetch_one(&pool)
                .await?;
        total_duration += dur;
    }

    assert_eq!(total_duration, 6);
    // Validate chaining: for every consecutive pair, ensure dependency exists
    for w in ids.windows(2) {
        let src = w[0];
        let tgt = w[1];
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM task_dependencies WHERE source_task_id = ? AND target_task_id = ?")
            .bind(src.to_string()).bind(tgt.to_string()).fetch_one(&pool).await?;
        assert_eq!(
            exists, 1,
            "consecutive pair {:?}->{:?} must be a dependency",
            src, tgt
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_zero_duration_tasks() -> anyhow::Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // Data: chain A->B->C with zero durations
    let user_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();

    sqlx::query("INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at) VALUES (?, 'T', 't@example.com', 'hash', 'local', datetime('now'), datetime('now'))")
        .bind(user_id.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, 'P', '', '#000', datetime('now'), datetime('now'))")
        .bind(project_id.to_string()).bind(user_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'A', 'todo', datetime('now'), datetime('now'))")
        .bind(a.to_string()).bind(project_id.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'B', 'todo', datetime('now'), datetime('now'))")
        .bind(b.to_string()).bind(project_id.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO tasks (id, project_id, title, status, created_at, updated_at) VALUES (?, ?, 'C', 'todo', datetime('now'), datetime('now'))")
        .bind(c.to_string()).bind(project_id.to_string()).execute(&pool).await?;

    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(a.to_string()).bind(b.to_string()).execute(&pool).await?;
    sqlx::query("INSERT INTO task_dependencies (id, source_task_id, target_task_id, created_at) VALUES (?, ?, ?, datetime('now'))")
        .bind(Uuid::new_v4().to_string()).bind(b.to_string()).bind(c.to_string()).execute(&pool).await?;

    use axum::extract::{Path as AxPath, State as AxState};
    use s_curve::app::AppState;
    use s_curve::jwt::{AuthUser, JwtConfig};
    use s_curve::routes::projects::get_project_critical_path;

    let jwt = JwtConfig {
        secret: std::sync::Arc::new(b"test-secret".to_vec()),
        exp_hours: 24,
    };
    let (event_bus, _rx) = tokio::sync::broadcast::channel(16);
    let app_state = AppState::new(
        pool.clone(),
        jwt,
        event_bus,
        s_curve::authz::RoutePermissionCache::new(),
    );
    let auth = AuthUser { user_id };

    let path = AxPath(project_id);
    let res = get_project_critical_path(AxState(app_state.clone()), auth.clone(), path).await?;
    let ids = res.0.task_ids;

    // All durations zero; algorithm maximizes sum of durations (0), so it may return
    // a single node or a chain. Accept any valid path with total duration 0 and
    // length between 1 and 3, and validate chaining.
    assert!(
        !ids.is_empty() && ids.len() <= 3,
        "unexpected path length: {}",
        ids.len()
    );

    // Ensure total duration is 0
    let mut total: i64 = 0;
    for id in ids.iter() {
        let dur: i64 =
            sqlx::query_scalar("SELECT COALESCE(duration_days, 0) FROM tasks WHERE id = ?")
                .bind(id.to_string())
                .fetch_one(&pool)
                .await?;
        total += dur;
    }
    assert_eq!(total, 0);

    // Validate chaining for consecutive pairs (if any)
    for w in ids.windows(2) {
        let src = w[0];
        let tgt = w[1];
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM task_dependencies WHERE source_task_id = ? AND target_task_id = ?")
            .bind(src.to_string()).bind(tgt.to_string()).fetch_one(&pool).await?;
        assert_eq!(
            exists, 1,
            "consecutive pair {:?}->{:?} must be a dependency",
            src, tgt
        );
    }

    Ok(())
}
