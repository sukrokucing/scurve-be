#![allow(clippy::uninlined_format_args)]

use anyhow::Context;
use anyhow::Result;
use axum::body::{self, Body};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::json;
use tower::util::ServiceExt; // for `oneshot`

use s_curve::create_app;

mod support;

#[tokio::test]
async fn full_api_flow() -> Result<()> {
    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    // create app router
    // tests run in CI/container; ensure a JWT secret is available for signing tokens
    std::env::set_var("JWT_SECRET", "test-secret");
    let app = create_app(pool.clone()).await?;

    // -- register
    let register_body = json!({
        "name": "Test User",
        "email": "test@example.com",
        "password": "password123"
    });

    let mut req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(register_body.to_string()))?;

    // Inject ConnectInfo for rate limiting
    use axum::extract::ConnectInfo;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 1234);
    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!(
            "register failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let auth_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let token = auth_res
        .get("token")
        .and_then(|v| v.as_str())
        .context("missing token")?
        .to_string();
    let user_id = auth_res
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|v| v.as_str())
        .context("missing user id")?
        .to_string();

    // Prefixing user_id with underscore to silence unused variable warning
    let _user_id = user_id.clone();

    // -- create project
    let project_body = json!({
        "name": "Test Project",
        "description": "desc",
        "theme_color": "#000000"
    });

    let mut req = Request::builder()
        .method("POST")
        .uri("/projects")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(project_body.to_string()))?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!(
            "project create failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let project_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let project_id = project_res
        .get("id")
        .and_then(|v| v.as_str())
        .context("missing project id")?
        .to_string();

    // -- create task
    let task_body = json!({
        "title": "Test Task",
        "status": "pending"
    });

    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/projects/{}/tasks", project_id))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(task_body.to_string()))?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!(
            "task create failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let task_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let task_id = task_res
        .get("id")
        .and_then(|v| v.as_str())
        .context("missing task id")?
        .to_string();

    // -- create progress
    let prog_body = json!({
        "progress": 50,
        "note": "Halfway"
    });

    let mut req = Request::builder()
        .method("POST")
        .uri(format!(
            "/projects/{}/tasks/{}/progress",
            project_id, task_id
        ))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(prog_body.to_string()))?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::CREATED {
        panic!(
            "progress create failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let prog_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let prog_id = prog_res
        .get("id")
        .and_then(|v| v.as_str())
        .context("missing progress id")?
        .to_string();

    // -- list progress
    let mut req = Request::builder()
        .method("GET")
        .uri(format!(
            "/projects/{}/tasks/{}/progress",
            project_id, task_id
        ))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!(
            "progress list failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let list_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    assert!(list_res.is_array());
    assert!(!list_res.as_array().unwrap().is_empty());

    // -- update progress
    let update_body = json!({"progress": 80});
    let mut req = Request::builder()
        .method("PUT")
        .uri(format!(
            "/projects/{}/tasks/{}/progress/{}",
            project_id, task_id, prog_id
        ))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(update_body.to_string()))?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!(
            "progress update failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }

    // -- delete progress
    let mut req = Request::builder()
        .method("DELETE")
        .uri(format!(
            "/projects/{}/tasks/{}/progress/{}",
            project_id, task_id, prog_id
        ))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    if status != StatusCode::NO_CONTENT {
        let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
        panic!(
            "progress delete failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }

    // -- list again should not include deleted
    let mut req = Request::builder()
        .method("GET")
        .uri(format!(
            "/projects/{}/tasks/{}/progress",
            project_id, task_id
        ))
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())?;

    req.extensions_mut().insert(ConnectInfo(addr));

    let resp: Response = app.clone().oneshot(req).await?;
    let status = resp.status();
    let body_bytes = body::to_bytes(resp.into_body(), 10_485_760).await?;
    if status != StatusCode::OK {
        panic!(
            "progress final list failed: {} - {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let list_res: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    // ensure deleted entries are filtered out
    assert!(!list_res
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.get("id").and_then(|x| x.as_str()) == Some(&prog_id)));

    Ok(())
}
