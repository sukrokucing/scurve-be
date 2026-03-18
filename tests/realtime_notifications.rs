#![allow(clippy::uninlined_format_args)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;
use uuid::Uuid;

use s_curve::{app, jwt};

mod support;

async fn setup() -> Result<(Router, SqlitePool, jwt::JwtConfig, support::db::TestDb)> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info,s_curve::routes::realtime=debug")
        .with_test_writer()
        .try_init();

    let test_db = support::db::cloned_clean_db().await?;
    let pool = test_db.pool.clone();

    sqlx::migrate!().run(&pool).await?;

    std::env::set_var("JWT_SECRET", "realtime-notifications-tests");
    std::env::remove_var("AUTHZ_MODE");
    std::env::set_var("SHOW_ERRORS", "1");
    seed_access_role_permissions(&pool).await?;
    let app = app::create_app(pool.clone()).await?;
    let jwt_config = jwt::JwtConfig {
        secret: Arc::new(b"realtime-notifications-tests".to_vec()),
        exp_hours: 1,
    };

    Ok((app, pool, jwt_config, test_db))
}

async fn seed_access_role_permissions(pool: &SqlitePool) -> Result<()> {
    for (role_name, permission_name) in [
        ("viewer", "project.view"),
        ("viewer", "task.view"),
        ("viewer", "progress.view"),
        ("member", "project.view"),
        ("member", "task.view"),
        ("member", "progress.create"),
        ("member", "project.update"),
        ("project_owner", "project.view"),
        ("project_owner", "task.view"),
        ("project_owner", "progress.view"),
    ] {
        sqlx::query(
            "INSERT OR IGNORE INTO role_permissions (role_id, permission_id, created_at)
             VALUES (
                (SELECT id FROM roles WHERE name = ?),
                (SELECT id FROM permissions WHERE name = ?),
                CURRENT_TIMESTAMP
             )",
        )
        .bind(role_name)
        .bind(permission_name)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn register_user(app: &Router, name: &str, email: &str) -> Result<(String, Uuid)> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": name,
                        "email": email,
                        "password": "password123"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let payload: Value = serde_json::from_slice(&body)?;
    let token = payload["token"]
        .as_str()
        .context("missing token")?
        .to_string();
    let user_id = payload["user"]["id"]
        .as_str()
        .context("missing user id")?
        .parse::<Uuid>()?;
    Ok((token, user_id))
}

async fn create_project(app: &Router, token: &str, name: &str) -> Result<Uuid> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": name,
                        "description": "desc",
                        "theme_color": "#224466"
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let payload: Value = serde_json::from_slice(&body)?;
    Ok(payload["id"]
        .as_str()
        .context("missing project id")?
        .parse()?)
}

async fn create_task(app: &Router, token: &str, project_id: Uuid, title: &str) -> Result<Uuid> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/tasks", project_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "title": title }).to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let payload: Value = serde_json::from_slice(&body)?;
    Ok(payload["id"].as_str().context("missing task id")?.parse()?)
}

async fn add_member(
    app: &Router,
    pool: &SqlitePool,
    owner_token: &str,
    project_id: Uuid,
    member_id: Uuid,
) -> Result<()> {
    let viewer_role_id: String =
        sqlx::query_scalar("SELECT id FROM roles WHERE name = 'viewer' LIMIT 1")
            .fetch_one(pool)
            .await?;
    add_member_with_access_role(
        app,
        pool,
        owner_token,
        project_id,
        member_id,
        &viewer_role_id,
    )
    .await
}

async fn add_member_with_access_role(
    app: &Router,
    pool: &SqlitePool,
    owner_token: &str,
    project_id: Uuid,
    member_id: Uuid,
    access_role_id: &str,
) -> Result<()> {
    let resource_role_id: String =
        sqlx::query_scalar("SELECT id FROM resource_roles WHERE name = 'unclassified' LIMIT 1")
            .fetch_one(pool)
            .await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{}/members", project_id))
                .header("Authorization", format!("Bearer {}", owner_token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "user_id": member_id,
                        "access_role_id": access_role_id,
                        "resource_role_ids": [resource_role_id]
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::CREATED);
    Ok(())
}

async fn create_access_role(pool: &SqlitePool, name: &str) -> Result<String> {
    let role_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO roles (id, name, description) VALUES (?, ?, ?)")
        .bind(&role_id)
        .bind(name)
        .bind("test role")
        .execute(pool)
        .await?;
    Ok(role_id)
}

async fn delete_member(
    app: &Router,
    owner_token: &str,
    project_id: Uuid,
    member_id: Uuid,
) -> Result<()> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{}/members/{}", project_id, member_id))
                .header("Authorization", format!("Bearer {}", owner_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    Ok(())
}

async fn wait_for_notifications(app: &Router, token: &str, expected: usize) -> Result<()> {
    for _ in 0..80 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/notifications")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())?,
            )
            .await?;
        let status = response.status();
        match status {
            StatusCode::OK => {
                let body = to_bytes(response.into_body(), usize::MAX).await?;
                let notifications: Value = serde_json::from_slice(&body)?;
                let count = notifications.as_array().map(Vec::len).unwrap_or_default();
                if count >= expected {
                    return Ok(());
                }
            }
            StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::NOT_FOUND => {
                let body = to_bytes(response.into_body(), usize::MAX).await?;
                anyhow::bail!(
                    "notification polling unexpectedly returned {}: {}",
                    status,
                    String::from_utf8_lossy(&body)
                );
            }
            _ => {}
        }

        if expected == 0 {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("timed out waiting for notifications")
}

async fn fetch_notifications(app: &Router, token: &str) -> Result<Vec<Value>> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/notifications")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let notifications: Value = serde_json::from_slice(&body)?;
    Ok(notifications.as_array().cloned().unwrap_or_default())
}

async fn spawn_server(app: Router) -> Result<(String, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr: SocketAddr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("server should run");
    });
    Ok((format!("ws://{}", addr), handle))
}

async fn next_ws_message(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<Message> {
    tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .context("timed out waiting for websocket message")?
        .context("websocket stream ended unexpectedly")?
        .map_err(Into::into)
}

#[tokio::test]
async fn websocket_accepts_query_token_and_subscribe_emits_presence() -> Result<()> {
    let (app, _pool, _jwt_config, _test_db) = setup().await?;
    let (owner_token, owner_id) =
        register_user(&app, "Realtime Owner", "rt-owner@example.com").await?;
    let project_id = create_project(&app, &owner_token, "Realtime Project").await?;

    let (base_url, server) = spawn_server(app.clone()).await?;
    let ws_url = format!("{}/realtime/ws?token={}", base_url, owner_token);
    let (mut socket, _response) = connect_async(ws_url).await?;

    let first = next_ws_message(&mut socket)
        .await
        .context("missing initial websocket event")?;
    let first = match first {
        Message::Text(text) => serde_json::from_str::<Value>(&text)?,
        other => anyhow::bail!("unexpected first websocket message: {other:?}"),
    };
    assert_eq!(first["family"], "notification");
    assert_eq!(first["change_type"], "snapshot");
    assert!(first["unread_count"].as_i64().unwrap_or(-1) >= 0);

    socket
        .send(Message::Text(
            json!({
                "type": "subscribe",
                "project_ids": [project_id],
                "route": "/tasks"
            })
            .to_string()
            .into(),
        ))
        .await?;

    let mut saw_snapshot = false;
    let mut saw_online = false;
    for _ in 0..6 {
        let message = next_ws_message(&mut socket)
            .await
            .context("missing subscribe response")?;
        let text = match message {
            Message::Text(text) => text,
            Message::Ping(_) | Message::Pong(_) => continue,
            other => anyhow::bail!("unexpected websocket message: {other:?}"),
        };
        let payload: Value = serde_json::from_str(&text)?;
        if payload["family"] == "presence" && payload["change_type"] == "snapshot" {
            assert_eq!(payload["metadata"]["user_id"], owner_id.to_string());
            assert_eq!(payload["metadata"]["status"], "online");
            assert_eq!(payload["metadata"]["route"], "/tasks");
            assert_eq!(
                payload["metadata"]["project_snapshot"][0]["user_id"],
                owner_id.to_string()
            );
            assert_eq!(
                payload["metadata"]["project_snapshot"][0]["name"],
                "Realtime Owner"
            );
            assert_eq!(
                payload["metadata"]["project_snapshot"][0]["route"],
                "/tasks"
            );
            saw_snapshot = true;
        }
        if payload["family"] == "presence" && payload["change_type"] == "online" {
            assert_eq!(payload["metadata"]["user_id"], owner_id.to_string());
            assert_eq!(payload["metadata"]["status"], "online");
            assert_eq!(payload["metadata"]["route"], "/tasks");
            saw_online = true;
        }
        if saw_snapshot && saw_online {
            break;
        }
    }

    assert!(saw_snapshot, "missing presence snapshot after subscribe");
    assert!(saw_online, "missing presence online event after subscribe");

    socket
        .send(Message::Text(
            json!({
                "type": "subscribe",
                "project_ids": [project_id],
                "route": "/dashboard"
            })
            .to_string()
            .into(),
        ))
        .await?;

    let mut saw_resnapshot = false;
    let mut saw_updated = false;
    for _ in 0..6 {
        let message = next_ws_message(&mut socket)
            .await
            .context("missing route update response")?;
        let text = match message {
            Message::Text(text) => text,
            Message::Ping(_) | Message::Pong(_) => continue,
            other => anyhow::bail!("unexpected websocket message: {other:?}"),
        };
        let payload: Value = serde_json::from_str(&text)?;
        if payload["family"] == "presence" && payload["change_type"] == "snapshot" {
            assert_eq!(payload["metadata"]["route"], "/dashboard");
            assert_eq!(
                payload["metadata"]["project_snapshot"][0]["route"],
                "/dashboard"
            );
            saw_resnapshot = true;
        }
        if payload["family"] == "presence" && payload["change_type"] == "updated" {
            assert_eq!(payload["metadata"]["status"], "online");
            assert_eq!(payload["metadata"]["route"], "/dashboard");
            saw_updated = true;
        }
        if saw_resnapshot && saw_updated {
            break;
        }
    }

    assert!(
        saw_resnapshot,
        "missing presence snapshot after route update"
    );
    assert!(
        saw_updated,
        "missing presence updated event after route update"
    );

    server.abort();
    let _ = server.await;
    Ok(())
}

#[tokio::test]
async fn task_changes_create_visible_notifications_and_read_endpoints_work() -> Result<()> {
    let (app, pool, _jwt_config, _test_db) = setup().await?;
    let (owner_token, _owner_id) = register_user(&app, "Owner", "notify-owner@example.com").await?;
    let (member_token, member_id) =
        register_user(&app, "Member", "notify-member@example.com").await?;

    let project_id = create_project(&app, &owner_token, "Notification Project").await?;
    add_member(&app, &pool, &owner_token, project_id, member_id).await?;
    assert!(s_curve::realtime::user_can_view_project(&pool, member_id, project_id).await?);
    let _task_id = create_task(
        &app,
        &owner_token,
        project_id,
        "Ship realtime notifications",
    )
    .await?;
    wait_for_notifications(&app, &member_token, 1).await?;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/notifications")
                .header("Authorization", format!("Bearer {}", member_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let notifications: Value = serde_json::from_slice(&body)?;
    let items = notifications
        .as_array()
        .context("notifications should be an array")?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["project_id"], project_id.to_string());
    assert_eq!(items[0]["entity_type"], "task");
    assert_eq!(items[0]["change_type"], "created");
    assert_eq!(items[0]["unread"], true);
    assert_eq!(items[0]["project_name"], "Notification Project");
    assert_eq!(
        items[0]["title"],
        "Task Created \"Ship realtime notifications\""
    );
    assert_eq!(
        items[0]["message"],
        "Owner created task \"Ship realtime notifications\" in Notification Project."
    );
    assert_eq!(
        items[0]["route"],
        format!(
            "/projects/{project_id}/tasks/{}",
            items[0]["entity_id"].as_str().unwrap()
        )
    );
    assert_eq!(items[0]["severity"], "important");
    let notification_id = items[0]["id"]
        .as_str()
        .context("missing notification id")?
        .to_string();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/notifications/unread-count")
                .header("Authorization", format!("Bearer {}", member_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let unread: Value = serde_json::from_slice(&body)?;
    assert_eq!(unread["unread_count"], 1);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/notifications/read")
                .header("Authorization", format!("Bearer {}", member_token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "ids": [notification_id] }).to_string()))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let read_payload: Value = serde_json::from_slice(&body)?;
    assert_eq!(read_payload["updated"], 1);
    assert_eq!(read_payload["unread_count"], 0);

    delete_member(&app, &owner_token, project_id, member_id).await?;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/notifications")
                .header("Authorization", format!("Bearer {}", member_token))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await?;
    let notifications: Value = serde_json::from_slice(&body)?;
    assert_eq!(notifications.as_array().map(Vec::len), Some(0));

    Ok(())
}

#[tokio::test]
async fn restricted_member_cannot_receive_notifications_or_subscribe_realtime() -> Result<()> {
    let (app, pool, _jwt_config, _test_db) = setup().await?;
    let (owner_token, _owner_id) = register_user(&app, "Owner 2", "owner-2@example.com").await?;
    let (member_token, member_id) =
        register_user(&app, "Restricted Member", "restricted-member@example.com").await?;

    let project_id = create_project(&app, &owner_token, "Restricted Visibility Project").await?;
    let restricted_role_id = create_access_role(&pool, "restricted_notifications_presence").await?;
    add_member_with_access_role(
        &app,
        &pool,
        &owner_token,
        project_id,
        member_id,
        &restricted_role_id,
    )
    .await?;
    assert!(!s_curve::realtime::user_can_view_project(&pool, member_id, project_id).await?);

    let _task_id = create_task(
        &app,
        &owner_token,
        project_id,
        "Invisible to restricted role",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let notifications = fetch_notifications(&app, &member_token).await?;
    assert!(
        notifications.is_empty(),
        "restricted member should not see notifications"
    );

    let (base_url, server) = spawn_server(app.clone()).await?;
    let ws_url = format!("{}/realtime/ws?token={}", base_url, member_token);
    let (mut socket, _response) = connect_async(ws_url).await?;

    let first = next_ws_message(&mut socket).await?;
    let first = match first {
        Message::Text(text) => serde_json::from_str::<Value>(&text)?,
        other => anyhow::bail!("unexpected first websocket message: {other:?}"),
    };
    assert_eq!(first["family"], "notification");
    assert_eq!(first["unread_count"], 0);

    socket
        .send(Message::Text(
            json!({
                "type": "subscribe",
                "project_ids": [project_id]
            })
            .to_string()
            .into(),
        ))
        .await?;

    let mut saw_denied = false;
    for _ in 0..4 {
        let message = next_ws_message(&mut socket).await?;
        let text = match message {
            Message::Text(text) => text,
            Message::Ping(_) | Message::Pong(_) => continue,
            other => anyhow::bail!("unexpected websocket message: {other:?}"),
        };
        let payload: Value = serde_json::from_str(&text)?;
        if payload["error"] == "bad_request"
            && payload["message"]
                .as_str()
                .unwrap_or_default()
                .contains("project subscription denied")
        {
            saw_denied = true;
            break;
        }
        assert_ne!(payload["family"], "presence");
    }

    assert!(
        saw_denied,
        "expected websocket subscribe denial for restricted member"
    );

    server.abort();
    let _ = server.await;
    Ok(())
}
