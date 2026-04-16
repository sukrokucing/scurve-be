//! Integration tests for:
//!   - GET /auth/me/permissions
//!   - X-View-As-User header (admin views as a specific user)
//!   - X-View-As-Role header (admin views as a generic role)
//!
//! NOTE: `cloned_clean_db()` truncates `role_permissions` and `user_roles`.
//! Tests must explicitly set up any permission grants they rely on.

#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use s_curve::create_app;
use s_curve::jwt::JwtConfig;
use serde_json::Value;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

const JWT_SECRET: &str = "test-secret-view-as-permissions";

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

async fn setup() -> Result<(axum::Router, SqlitePool, support::db::TestDb)> {
    std::env::set_var("JWT_SECRET", JWT_SECRET);
    std::env::set_var("AUTHZ_MODE", "strict");

    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();
    let app = create_app(pool.clone()).await?;

    Ok((app, pool, test_db))
}

fn make_token(user_id: Uuid) -> String {
    let jwt = JwtConfig {
        secret: Arc::new(JWT_SECRET.as_bytes().to_vec()),
        exp_hours: 1,
    };
    jwt.encode(user_id).unwrap()
}

/// Insert a bare user row and return their id.
async fn insert_user(pool: &SqlitePool, label: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO users (id, name, email, password_hash, provider, created_at, updated_at)
         VALUES (?, ?, ?, 'hash', 'local', ?, ?)",
    )
    .bind(user_id.to_string())
    .bind(label)
    .bind(format!("{}-{}@test.com", label.replace(' ', "-"), &user_id.to_string()[..8]))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    user_id
}

/// Record a role-name membership for a user (populates `user_roles`).
/// This makes `principal.has_role(name)` return true, but grants NO permissions
/// unless you also call `setup_role_permissions` for that role.
async fn assign_role(pool: &SqlitePool, user_id: Uuid, role_name: &str) {
    let role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE name = ?")
        .bind(role_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("seeded role '{}' not found", role_name));

    sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id.to_string())
        .bind(role_id)
        .execute(pool)
        .await
        .unwrap();
}

/// Grant permissions directly to a user via `user_permissions` (no scope).
async fn grant_user_permissions(pool: &SqlitePool, user_id: Uuid, perm_names: &[&str]) {
    let now = chrono::Utc::now();
    for perm_name in perm_names {
        let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
            .bind(perm_name)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| panic!("seeded permission '{}' not found", perm_name));

        sqlx::query(
            "INSERT INTO user_permissions (id, user_id, permission_id, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id.to_string())
        .bind(perm_id)
        .bind(now)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Populate `role_permissions` for a named role.
/// Required when tests depend on permissions derived from role membership
/// (e.g. view-as-role, or project_members access_role_id lookups).
async fn setup_role_permissions(pool: &SqlitePool, role_name: &str, perm_names: &[&str]) {
    let role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE name = ?")
        .bind(role_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("seeded role '{}' not found", role_name));

    for perm_name in perm_names {
        let perm_id: String = sqlx::query_scalar("SELECT id FROM permissions WHERE name = ?")
            .bind(perm_name)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| panic!("seeded permission '{}' not found", perm_name));

        sqlx::query(
            "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES (?, ?)",
        )
        .bind(&role_id)
        .bind(perm_id)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Insert a project row. Returns the new project_id.
async fn insert_project(pool: &SqlitePool, owner_id: Uuid) -> Uuid {
    let project_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO projects (id, user_id, name, created_at, updated_at)
         VALUES (?, ?, 'Test Project', ?, ?)",
    )
    .bind(project_id.to_string())
    .bind(owner_id.to_string())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    project_id
}

/// Add a user as a project member with the given role (uses access_role_id).
async fn add_project_member(pool: &SqlitePool, project_id: Uuid, user_id: Uuid, role_name: &str) {
    let member_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE name = ?")
        .bind(role_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("seeded role '{}' not found", role_name));

    sqlx::query(
        "INSERT INTO project_members
         (id, project_id, user_id, access_role_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(member_id.to_string())
    .bind(project_id.to_string())
    .bind(user_id.to_string())
    .bind(role_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

/// GET with optional extra headers. Returns (status, parsed JSON body).
async fn get_with_headers(
    app: &axum::Router,
    uri: &str,
    token: &str,
    extra: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", format!("Bearer {}", token));

    for (k, v) in extra {
        builder = builder.header(*k, *v);
    }

    let req = builder.body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1_000_000).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, Value) {
    get_with_headers(app, uri, token, &[]).await
}

// ---------------------------------------------------------------------------
// GET /auth/me/permissions
// ---------------------------------------------------------------------------

/// Viewer role (populated via role_permissions) returns correct global permissions.
#[tokio::test]
async fn me_permissions_returns_global_role_permissions() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // Populate role_permissions for viewer (cleared by cloned_clean_db)
    setup_role_permissions(&pool, "viewer", &["project.view", "task.view", "progress.view"]).await;

    let viewer_id = insert_user(&pool, "Viewer").await;
    assign_role(&pool, viewer_id, "viewer").await;
    let token = make_token(viewer_id);

    let (status, body) = get(&app, "/auth/me/permissions", &token).await;
    assert_eq!(status, StatusCode::OK);

    let roles = body["roles"].as_array().unwrap();
    let perms = body["permissions"].as_array().unwrap();
    let proj_perms = body["project_permissions"].as_array().unwrap();

    assert!(
        roles.iter().any(|r| r == "viewer"),
        "roles must contain 'viewer', got: {:?}", roles
    );
    assert!(
        perms.iter().any(|p| p == "project.view"),
        "viewer must have project.view, got: {:?}", perms
    );
    assert!(
        perms.iter().any(|p| p == "task.view"),
        "viewer must have task.view, got: {:?}", perms
    );
    assert!(
        perms.iter().any(|p| p == "progress.view"),
        "viewer must have progress.view, got: {:?}", perms
    );
    assert!(
        proj_perms.is_empty(),
        "no project memberships yet, got: {:?}", proj_perms
    );

    Ok(())
}

/// Project membership permissions appear in project_permissions, not global permissions.
#[tokio::test]
async fn me_permissions_includes_project_memberships() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // Populate role_permissions for viewer — required for project_members → role_permissions join
    setup_role_permissions(&pool, "viewer", &["project.view", "task.view", "progress.view"]).await;

    let owner_id = insert_user(&pool, "Owner").await;
    let member_id = insert_user(&pool, "Member").await; // no global role

    let project_id = insert_project(&pool, owner_id).await;
    add_project_member(&pool, project_id, member_id, "viewer").await;

    let token = make_token(member_id);
    let (status, body) = get(&app, "/auth/me/permissions", &token).await;
    assert_eq!(status, StatusCode::OK);

    let global_perms = body["permissions"].as_array().unwrap();
    let proj_perms = body["project_permissions"].as_array().unwrap();

    assert!(
        global_perms.is_empty(),
        "no global permissions — membership is project-scoped, got: {:?}", global_perms
    );
    assert_eq!(proj_perms.len(), 1, "should have exactly one project, got: {:?}", proj_perms);

    let entry = &proj_perms[0];
    assert_eq!(
        entry["project_id"].as_str().unwrap(),
        project_id.to_string(),
        "project_id must match"
    );
    let perm_list = entry["permissions"].as_array().unwrap();
    assert!(
        perm_list.iter().any(|p| p == "project.view"),
        "project membership as viewer grants project.view, got: {:?}", perm_list
    );
    assert!(
        perm_list.iter().any(|p| p == "task.view"),
        "project membership as viewer grants task.view, got: {:?}", perm_list
    );

    Ok(())
}

/// Calling /auth/me/permissions without a token returns 401.
#[tokio::test]
async fn me_permissions_requires_auth() -> Result<()> {
    let (app, _pool, _db) = setup().await?;

    let req = Request::builder()
        .method("GET")
        .uri("/auth/me/permissions")
        .body(Body::empty())?;
    let res = app.oneshot(req).await?;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

// ---------------------------------------------------------------------------
// X-View-As-User
// ---------------------------------------------------------------------------

/// Admin viewing as a viewer gains the viewer's permissions and loses their own.
#[tokio::test]
async fn view_as_user_restricts_admin_to_target_users_permissions() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // Admin: has_role("admin") for view-as guard + direct role.view permission
    let admin_id = insert_user(&pool, "Admin").await;
    assign_role(&pool, admin_id, "admin").await;
    grant_user_permissions(&pool, admin_id, &["role.view"]).await;

    // Viewer: direct project.view — no role.view
    let viewer_id = insert_user(&pool, "Viewer").await;
    grant_user_permissions(&pool, viewer_id, &["project.view"]).await;

    let admin_token = make_token(admin_id);
    let viewer_id_str = viewer_id.to_string();

    // Baseline: admin can list RBAC roles, cannot list projects
    let (status, _) = get(&app, "/rbac/roles", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "admin should have role.view normally");

    let (status, _) = get(&app, "/projects", &admin_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "admin has no project.view normally");

    // View-as viewer: gains project.view, loses role.view
    let (status, _) = get_with_headers(
        &app, "/projects", &admin_token,
        &[("X-View-As-User", viewer_id_str.as_str())],
    ).await;
    assert_eq!(status, StatusCode::OK, "view-as viewer → /projects allowed");

    let (status, _) = get_with_headers(
        &app, "/rbac/roles", &admin_token,
        &[("X-View-As-User", viewer_id_str.as_str())],
    ).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "view-as viewer → /rbac/roles denied");

    Ok(())
}

/// When viewing as a user, their project-scoped permissions are respected.
#[tokio::test]
async fn view_as_user_includes_project_scoped_permissions() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // Populate role_permissions for viewer (required for project_members lookup)
    setup_role_permissions(&pool, "viewer", &["project.view", "task.view", "progress.view"]).await;

    let admin_id = insert_user(&pool, "Admin").await;
    assign_role(&pool, admin_id, "admin").await; // for view-as guard

    let viewer_id = insert_user(&pool, "Viewer").await; // no global role

    let project_a = insert_project(&pool, admin_id).await;
    let project_b = insert_project(&pool, admin_id).await;
    add_project_member(&pool, project_a, viewer_id, "viewer").await; // A only

    let admin_token = make_token(admin_id);
    let viewer_id_str = viewer_id.to_string();

    // View-as viewer: project A tasks accessible (task.view scoped to A)
    let (status, _) = get_with_headers(
        &app,
        &format!("/projects/{}/tasks", project_a),
        &admin_token,
        &[("X-View-As-User", viewer_id_str.as_str())],
    ).await;
    assert_eq!(status, StatusCode::OK, "viewer has task.view scoped to project A");

    // View-as viewer: project B tasks denied (no membership)
    let (status, _) = get_with_headers(
        &app,
        &format!("/projects/{}/tasks", project_b),
        &admin_token,
        &[("X-View-As-User", viewer_id_str.as_str())],
    ).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "viewer has no access to project B");

    Ok(())
}

// ---------------------------------------------------------------------------
// X-View-As-Role
// ---------------------------------------------------------------------------

/// Admin viewing as a role gains that role's permissions and loses their own.
#[tokio::test]
async fn view_as_role_restricts_admin_to_role_permissions() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // Populate role_permissions for viewer — but NOT role.view
    setup_role_permissions(&pool, "viewer", &["project.view", "task.view", "progress.view"]).await;

    let admin_id = insert_user(&pool, "Admin").await;
    assign_role(&pool, admin_id, "admin").await;
    grant_user_permissions(&pool, admin_id, &["role.view"]).await;

    let admin_token = make_token(admin_id);

    // View-as "viewer" role: gains project.view, loses role.view
    let (status, _) = get_with_headers(
        &app, "/projects", &admin_token,
        &[("X-View-As-Role", "viewer")],
    ).await;
    assert_eq!(status, StatusCode::OK, "view-as viewer role → /projects allowed");

    let (status, _) = get_with_headers(
        &app, "/rbac/roles", &admin_token,
        &[("X-View-As-Role", "viewer")],
    ).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "view-as viewer role → /rbac/roles denied");

    Ok(())
}

/// An unknown role name is silently ignored — real permissions apply.
#[tokio::test]
async fn view_as_unknown_role_falls_back_to_real_permissions() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let admin_id = insert_user(&pool, "Admin").await;
    assign_role(&pool, admin_id, "admin").await;
    grant_user_permissions(&pool, admin_id, &["role.view"]).await;

    let admin_token = make_token(admin_id);

    let (status, _) = get_with_headers(
        &app, "/rbac/roles", &admin_token,
        &[("X-View-As-Role", "nonexistent_role_xyz")],
    ).await;
    assert_eq!(
        status, StatusCode::OK,
        "unknown role falls back to real admin permissions (role.view)"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Non-admin cannot use view-as
// ---------------------------------------------------------------------------

/// Non-admin users have X-View-As-Role silently ignored.
#[tokio::test]
async fn view_as_ignored_for_non_admin_user() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    // Populate super_admin role_permissions so view-as-role super_admin would bypass if honoured
    // (super_admin bypass is in the evaluator: is_super_admin() = true → allow all)
    // No need to populate — super_admin bypass is based on role name, not role_permissions.

    // project_manager: not admin/super_admin, has project.view but not role.view
    let pm_id = insert_user(&pool, "PM").await;
    assign_role(&pool, pm_id, "project_manager").await; // not admin — guard ignored
    grant_user_permissions(&pool, pm_id, &["project.view"]).await;

    let pm_token = make_token(pm_id);

    // PM tries to view-as super_admin role — header is silently ignored
    // If honoured: is_super_admin() → true → /rbac/roles → 200
    // If ignored: PM has no role.view → /rbac/roles → 403
    let (status, _) = get_with_headers(
        &app, "/rbac/roles", &pm_token,
        &[("X-View-As-Role", "super_admin")],
    ).await;
    assert_eq!(
        status, StatusCode::FORBIDDEN,
        "non-admin view-as must be silently ignored"
    );

    // PM's own permissions still work
    let (status, _) = get(&app, "/projects", &pm_token).await;
    assert_eq!(status, StatusCode::OK, "PM still has project.view normally");

    Ok(())
}

// ---------------------------------------------------------------------------
// Header priority: X-View-As-User beats X-View-As-Role
// ---------------------------------------------------------------------------

/// When both headers are sent, X-View-As-User takes priority over X-View-As-Role.
#[tokio::test]
async fn view_as_user_takes_priority_over_view_as_role() -> Result<()> {
    let (app, pool, _db) = setup().await?;

    let admin_id = insert_user(&pool, "Admin").await;
    assign_role(&pool, admin_id, "admin").await;

    // Viewer user: no permissions (role_permissions cleared, no user_permissions)
    let viewer_id = insert_user(&pool, "Viewer").await;
    assign_role(&pool, viewer_id, "viewer").await; // viewer role but no role_permissions

    let admin_token = make_token(admin_id);
    let viewer_id_str = viewer_id.to_string();

    // X-View-As-User=viewer (no permissions) + X-View-As-Role=super_admin (bypass all)
    // If X-View-As-User wins → viewer has no role.view → /rbac/roles → 403  ✓
    // If X-View-As-Role wins → super_admin bypass → /rbac/roles → 200  ✗
    let (status, _) = get_with_headers(
        &app, "/rbac/roles", &admin_token,
        &[
            ("X-View-As-User", viewer_id_str.as_str()),
            ("X-View-As-Role", "super_admin"),
        ],
    ).await;
    assert_eq!(
        status, StatusCode::FORBIDDEN,
        "X-View-As-User must take priority over X-View-As-Role"
    );

    Ok(())
}
