use s_curve::models::user::{RegisterRequest, AuthResponse};
use s_curve::models::project::Project;
use s_curve::models::rbac::GrantPermissionRequest;
use s_curve::models::project::ProjectCreateRequest;
use sqlx::SqlitePool;
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt;
use serde_json::json;

async fn setup() -> (Router, SqlitePool, String) {
    // Set up env
    let _ = tracing_subscriber::fmt()
        .with_env_filter("s_curve=trace,axum=trace,tower_http=trace")
        .try_init();

    std::env::set_var("JWT_SECRET", "test_secret_key_scoped_rbac");
    std::env::set_var("AUTHZ_MODE", "strict"); // Important: enforce strict mode

    let _ = dotenvy::dotenv();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let (tx, _rx) = s_curve::events::init_event_bus();
    let state = s_curve::app::AppState {
        pool: pool.clone(),
        jwt: Arc::new(s_curve::jwt::JwtConfig {
            secret: Arc::new("test_secret_key_scoped_rbac".as_bytes().to_vec()),
            exp_hours: 1,
        }),
        event_bus: tx,
    };

    let app = s_curve::app::api_routes(state);

    // Create super admin to set things up
    let admin_token = create_user(&app, "Admin", "admin@example.com").await;

    // Assign super_admin role to admin (assumes role exists from migration or we create it)
    // Actually migration creates roles but not super_admin assignment.
    // Let's manually insert role assignment for test simplicity or use API

    // Find super_admin role
    let role_id = fetch_id(&pool, "roles", "name", "super_admin").await;

    // Get user id
    let user_id = fetch_id(&pool, "users", "email", "admin@example.com").await;

    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id.to_string())
        .bind(role_id.to_string())
        .execute(&pool).await.expect("Failed to insert user_role");

    (app, pool, admin_token)
}

fn insert_hyphens(s: &str) -> String {
    format!("{}-{}-{}-{}-{}", &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32])
}

async fn fetch_id(pool: &SqlitePool, table: &str, field: &str, val: &str) -> uuid::Uuid {
    let sql = format!("SELECT CASE WHEN typeof(id)='blob' THEN lower(hex(id)) ELSE id END FROM {} WHERE {} = ?", table, field);
    let id_str: String = sqlx::query_scalar(&sql)
        .bind(val)
        .fetch_one(pool)
        .await
        .unwrap();

    if id_str.len() == 32 {
        uuid::Uuid::parse_str(&insert_hyphens(&id_str)).unwrap()
    } else {
        uuid::Uuid::parse_str(&id_str).unwrap()
    }
}

async fn create_user(app: &Router, name: &str, email: &str) -> String {
    let payload = RegisterRequest {
        name: name.to_string(),
        email: email.to_string(),
        password: "Password123!".to_string(),
    };

    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(res.into_body(), 10000).await.unwrap();
    let auth: AuthResponse = serde_json::from_slice(&body).unwrap();
    auth.token
}

async fn create_project(app: &Router, token: &str, name: &str) -> String {
    let payload = ProjectCreateRequest {
        name: name.to_string(),
        description: None,
        theme_color: None,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(res.into_body(), 10000).await.unwrap();
    let project: Project = serde_json::from_slice(&body).unwrap();
    project.id.to_string()
}

#[tokio::test]
async fn test_scoped_permission_enforcement() {
    let (app, pool, admin_token) = setup().await;

    // 1. Create two projects
    let project_a_id = create_project(&app, &admin_token, "Project A").await;
    let project_b_id = create_project(&app, &admin_token, "Project B").await;

    // 2. Create a restricted user
    let user_token = create_user(&app, "User", "user@example.com").await;
    let user_id = fetch_id(&pool, "users", "email", "user@example.com").await;

    // 3. Grant `project.view` permission ONLY for Project A
    let perm_id = fetch_id(&pool, "permissions", "name", "project.view").await;

    let scope = json!({ "project_id": project_a_id });

    let grant_req = GrantPermissionRequest {
        permission_id: perm_id,
        scope: Some(scope),
    };

    let req = Request::builder()
        .method("POST")
        .uri(&format!("/rbac/users/{}/permissions", user_id))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_string(&grant_req).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 4. Try to access Project A tasks (Should be ALLOWED)
    // Assuming GET /projects/:id/tasks requires `task.view`? No, usually `project.view` allows viewing project things.
    // Check middleware on `list_tasks`. It requires `task.view`.
    // Wait, I granted `project.view`, not `task.view`. Let's grant `task.view` scoped.

    let perm_task_view_id = fetch_id(&pool, "permissions", "name", "task.view").await;

    let grant_req_task = GrantPermissionRequest {
        permission_id: perm_task_view_id,
        scope: Some(json!({ "project_id": project_a_id })),
    };

    let req = Request::builder()
        .method("POST")
        .uri(&format!("/rbac/users/{}/permissions", user_id))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_string(&grant_req_task).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);


    // Access Project A tasks
    let req = Request::builder()
        .method("GET")
        .uri(&format!("/projects/{}/tasks", project_a_id))
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "Should be allowed to view Project A tasks");

    // 5. Try to access Project B tasks (Should be DENIED)
    let req = Request::builder()
        .method("GET")
        .uri(&format!("/projects/{}/tasks", project_b_id))
        .header("Authorization", format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "Should be denied view Project B tasks");
}
