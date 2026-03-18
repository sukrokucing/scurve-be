use std::collections::HashSet;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::realtime::{RealtimeClientCommand, RealtimeEvent, RealtimeEventFamily};
use crate::realtime::{
    count_visible_unread_notifications, fetch_actor_info, system_actor, user_can_view_project,
    RealtimeOutbound,
};

const HEARTBEAT_SECONDS: u64 = 25;

#[derive(Debug, Deserialize, ToSchema)]
pub struct RealtimeWsQuery {
    #[allow(dead_code)]
    #[schema(example = "<jwt-token>")]
    pub token: Option<String>,
}

#[utoipa::path(
    get,
    path = "/realtime/ws",
    tag = "Realtime",
    params(("token" = Option<String>, Query, description = "Optional JWT bearer token query fallback for browser websocket clients. Authorization header remains supported.")),
    responses(
        (status = 200, description = "WebSocket upgrade endpoint"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("bearerAuth" = []))
)]
pub async fn websocket_feed(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(_query): Query<RealtimeWsQuery>,
    ws: WebSocketUpgrade,
) -> AppResult<Response> {
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, auth.user_id)))
}

async fn handle_socket(socket: WebSocket, state: AppState, user_id: Uuid) {
    let actor = fetch_actor_info(&state.pool, user_id)
        .await
        .unwrap_or_else(|_| system_actor());
    let (connection_id, mut rx) = state
        .realtime_hub
        .register_connection(user_id, actor.clone())
        .await;

    let unread_count = count_visible_unread_notifications(&state.pool, user_id)
        .await
        .unwrap_or(0);
    state
        .realtime_hub
        .send_to_connection(
            connection_id,
            RealtimeEvent {
                family: RealtimeEventFamily::Notification,
                project_id: None,
                event_id: Uuid::new_v4(),
                actor: system_actor(),
                entity_type: "notification".to_string(),
                entity_id: None,
                change_type: "snapshot".to_string(),
                occurred_at: chrono::Utc::now(),
                unread_count: Some(unread_count),
                metadata: None,
            },
        )
        .await;

    let (mut sender, mut receiver) = socket.split();
    let writer = tokio::spawn(async move {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECONDS));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    if sender.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
                outbound = rx.recv() => {
                    let Some(outbound) = outbound else {
                        break;
                    };
                    let send_result = match outbound {
                        RealtimeOutbound::Event(event) => send_json_message(&mut sender, &event).await,
                        RealtimeOutbound::Error(error) => send_json_message(&mut sender, &error).await,
                        RealtimeOutbound::Ping => async_send(&mut sender, Message::Ping(Vec::new().into())).await,
                    };

                    if send_result.is_err() {
                        break;
                    }
                }
            }
        }
    });

    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Text(text)) => {
                tracing::debug!(user_id = %user_id, payload = %text, "realtime text message received");
                if let Err(error) =
                    handle_command(&state, user_id, connection_id, text.as_str()).await
                {
                    let _ = state
                        .realtime_hub
                        .send_error(connection_id, error.to_string())
                        .await;
                }
            }
            Ok(Message::Binary(_)) => {
                state
                    .realtime_hub
                    .send_error(connection_id, "binary websocket messages are not supported")
                    .await;
            }
            Ok(Message::Ping(payload)) => {
                let _ = state
                    .realtime_hub
                    .send_ping_to_connection(connection_id)
                    .await;
                // best-effort pong via hub ping frame above; payload is ignored for now
                let _ = payload;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => break,
            Err(error) => {
                tracing::debug!(user_id = %user_id, "websocket receive error: {}", error);
                break;
            }
        }
    }

    state
        .realtime_hub
        .unregister_connection(connection_id)
        .await;
    writer.abort();
    let _ = writer.await;
}

async fn async_send(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: Message,
) -> Result<(), String> {
    sender
        .send(message)
        .await
        .map_err(|error| error.to_string())
}

async fn send_json_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    value: &impl serde::Serialize,
) -> Result<(), String> {
    let payload = serde_json::to_string(value).map_err(|error| error.to_string())?;
    async_send(sender, Message::Text(payload.into())).await
}

async fn handle_command(
    state: &AppState,
    user_id: Uuid,
    connection_id: Uuid,
    text: &str,
) -> AppResult<()> {
    let command: RealtimeClientCommand = serde_json::from_str(text)
        .map_err(|error| AppError::bad_request(format!("invalid realtime command: {}", error)))?;

    match command {
        RealtimeClientCommand::Subscribe { project_ids, route } => {
            let project_ids = dedupe_project_ids(project_ids);
            let route = normalize_presence_route(route)?;
            let (allowed, denied) =
                filter_subscribable_projects(state, user_id, &project_ids).await?;
            tracing::debug!(
                user_id = %user_id,
                allowed = ?allowed,
                denied = ?denied,
                route = ?route,
                "realtime subscribe command processed"
            );
            if !allowed.is_empty() {
                state
                    .realtime_hub
                    .subscribe_projects(connection_id, &allowed, route)
                    .await;
            }
            if !denied.is_empty() {
                let denied = denied
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                state
                    .realtime_hub
                    .send_error(
                        connection_id,
                        format!("project subscription denied for: {denied}"),
                    )
                    .await;
            }
        }
        RealtimeClientCommand::Unsubscribe { project_ids } => {
            let project_ids = dedupe_project_ids(project_ids);
            tracing::debug!(user_id = %user_id, project_ids = ?project_ids, "realtime unsubscribe command processed");
            state
                .realtime_hub
                .unsubscribe_projects(connection_id, &project_ids)
                .await;
        }
        RealtimeClientCommand::Ping => {
            state
                .realtime_hub
                .send_ping_to_connection(connection_id)
                .await;
        }
    }

    Ok(())
}

fn normalize_presence_route(route: Option<String>) -> AppResult<Option<String>> {
    let Some(route) = route else {
        return Ok(None);
    };

    let trimmed = route.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 512 {
        return Err(AppError::bad_request(
            "realtime route must be 512 characters or fewer",
        ));
    }

    Ok(Some(trimmed.to_string()))
}

fn dedupe_project_ids(project_ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for project_id in project_ids {
        if seen.insert(project_id) {
            deduped.push(project_id);
        }
    }
    deduped
}

async fn filter_subscribable_projects(
    state: &AppState,
    user_id: Uuid,
    project_ids: &[Uuid],
) -> AppResult<(Vec<Uuid>, Vec<Uuid>)> {
    let mut allowed = Vec::new();
    let mut denied = Vec::new();
    for project_id in project_ids {
        let can_view = user_can_view_project(&state.pool, user_id, *project_id).await?;
        if can_view {
            allowed.push(*project_id);
        } else {
            denied.push(*project_id);
        }
    }

    Ok((allowed, denied))
}
