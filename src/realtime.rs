use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use crate::authz::{DefaultPolicyEvaluator, PolicyEvaluator, Principal, ResourceContext};
use crate::db::uuid_sql;
use crate::errors::{AppError, AppResult};
use crate::models::notification::{NotificationActor, NotificationSeverity};
use crate::models::realtime::{
    PresenceStatus, RealtimeActor, RealtimeErrorMessage, RealtimeEvent, RealtimeEventFamily,
    RealtimeEventMetadata, RealtimeInvalidateMetadata, RealtimePresenceMetadata,
    RealtimePresenceUser,
};

#[derive(Debug, Clone)]
pub enum RealtimeOutbound {
    Event(RealtimeEvent),
    Error(RealtimeErrorMessage),
    Ping,
}

#[derive(Default)]
struct HubState {
    connections: HashMap<Uuid, ConnectionState>,
    user_connections: HashMap<Uuid, HashSet<Uuid>>,
    user_actors: HashMap<Uuid, RealtimeActor>,
    project_subscribers: HashMap<Uuid, HashSet<Uuid>>,
    project_presence: HashMap<Uuid, HashMap<Uuid, PresenceState>>,
}

struct ConnectionState {
    user_id: Uuid,
    sender: mpsc::UnboundedSender<RealtimeOutbound>,
    subscribed_projects: HashSet<Uuid>,
    route: Option<String>,
}

#[derive(Debug, Clone)]
struct PresenceState {
    connections: usize,
    route: Option<String>,
    last_seen_at: DateTime<Utc>,
}

#[derive(Clone, Default)]
pub struct RealtimeHub {
    inner: Arc<RwLock<HubState>>,
}

impl RealtimeHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_connection(
        &self,
        user_id: Uuid,
        actor: RealtimeActor,
    ) -> (Uuid, mpsc::UnboundedReceiver<RealtimeOutbound>) {
        let connection_id = Uuid::new_v4();
        let (tx, rx) = mpsc::unbounded_channel();
        let mut inner = self.inner.write().await;
        inner.connections.insert(
            connection_id,
            ConnectionState {
                user_id,
                sender: tx,
                subscribed_projects: HashSet::new(),
                route: None,
            },
        );
        inner
            .user_connections
            .entry(user_id)
            .or_default()
            .insert(connection_id);
        inner.user_actors.insert(user_id, actor);
        (connection_id, rx)
    }

    pub async fn send_to_connection(&self, connection_id: Uuid, event: RealtimeEvent) {
        let sender = {
            let inner = self.inner.read().await;
            inner
                .connections
                .get(&connection_id)
                .map(|connection| connection.sender.clone())
        };

        if let Some(sender) = sender {
            let _ = sender.send(RealtimeOutbound::Event(event));
        }
    }

    pub async fn unregister_connection(&self, connection_id: Uuid) {
        let (presence_events, targets) = {
            let mut inner = self.inner.write().await;
            let Some(connection) = inner.connections.remove(&connection_id) else {
                return;
            };
            let actor = inner
                .user_actors
                .get(&connection.user_id)
                .cloned()
                .unwrap_or_else(system_actor);

            if let Some(ids) = inner.user_connections.get_mut(&connection.user_id) {
                ids.remove(&connection_id);
                if ids.is_empty() {
                    inner.user_connections.remove(&connection.user_id);
                    inner.user_actors.remove(&connection.user_id);
                }
            }

            let mut events = Vec::new();
            for project_id in connection.subscribed_projects {
                if let Some(subscribers) = inner.project_subscribers.get_mut(&project_id) {
                    subscribers.remove(&connection_id);
                    if subscribers.is_empty() {
                        inner.project_subscribers.remove(&project_id);
                    }
                }

                let removed_presence =
                    decrement_presence(&mut inner.project_presence, project_id, connection.user_id);
                if let Some(removed_presence) = removed_presence {
                    events.push(build_presence_event(
                        project_id,
                        actor.clone(),
                        "project",
                        Some(project_id),
                        "offline",
                        removed_presence.route,
                        None,
                    ));
                }
            }

            let targets = collect_project_targets(
                &inner,
                events
                    .iter()
                    .filter_map(|event| event.project_id)
                    .collect::<Vec<_>>(),
            );
            (events, targets)
        };

        send_events(targets, presence_events);
    }

    pub async fn subscribe_projects(
        &self,
        connection_id: Uuid,
        project_ids: &[Uuid],
        route: Option<String>,
    ) {
        let (direct_sender, direct_events, broadcast_events, targets) = {
            let mut inner = self.inner.write().await;
            let Some(user_id) = inner
                .connections
                .get(&connection_id)
                .map(|connection| connection.user_id)
            else {
                return;
            };
            let direct_sender = inner
                .connections
                .get(&connection_id)
                .map(|connection| connection.sender.clone());

            let actor = inner
                .user_actors
                .get(&user_id)
                .cloned()
                .unwrap_or_else(system_actor);
            let mut direct_events = Vec::new();
            let mut broadcast_events = Vec::new();
            let mut previous_route = inner
                .connections
                .get(&connection_id)
                .and_then(|connection| connection.route.clone());

            if let Some(connection) = inner.connections.get_mut(&connection_id) {
                if let Some(route) = route.clone() {
                    connection.route = Some(route);
                }
            }

            for project_id in project_ids {
                let Some(already_subscribed) = inner
                    .connections
                    .get_mut(&connection_id)
                    .map(|connection| !connection.subscribed_projects.insert(*project_id))
                else {
                    continue;
                };
                let effective_route = route.clone().or_else(|| previous_route.clone());
                let presence_change = if already_subscribed {
                    refresh_presence(
                        &mut inner.project_presence,
                        *project_id,
                        user_id,
                        effective_route.clone(),
                    )
                } else {
                    inner
                        .project_subscribers
                        .entry(*project_id)
                        .or_default()
                        .insert(connection_id);
                    increment_presence(
                        &mut inner.project_presence,
                        *project_id,
                        user_id,
                        effective_route.clone(),
                    )
                };
                previous_route = presence_change.route.clone();

                let online_users = snapshot_online_users(&inner, *project_id);
                direct_events.push(build_presence_event(
                    *project_id,
                    actor.clone(),
                    "project",
                    Some(*project_id),
                    "snapshot",
                    presence_change.route.clone(),
                    Some(online_users),
                ));

                if presence_change.first_connection {
                    broadcast_events.push(build_presence_event(
                        *project_id,
                        actor.clone(),
                        "project",
                        Some(*project_id),
                        "online",
                        presence_change.route,
                        None,
                    ));
                } else if presence_change.route_changed {
                    broadcast_events.push(build_presence_event(
                        *project_id,
                        actor.clone(),
                        "project",
                        Some(*project_id),
                        "updated",
                        presence_change.route,
                        None,
                    ));
                }
            }

            let project_ids: Vec<Uuid> = broadcast_events
                .iter()
                .filter_map(|event| event.project_id)
                .collect();
            let targets = collect_project_targets(&inner, project_ids);
            (direct_sender, direct_events, broadcast_events, targets)
        };

        if let Some(sender) = direct_sender {
            for event in direct_events {
                let _ = sender.send(RealtimeOutbound::Event(event));
            }
        }
        send_events(targets, broadcast_events);
    }

    pub async fn unsubscribe_projects(&self, connection_id: Uuid, project_ids: &[Uuid]) {
        let (events, targets) = {
            let mut inner = self.inner.write().await;
            let Some(user_id) = inner
                .connections
                .get(&connection_id)
                .map(|connection| connection.user_id)
            else {
                return;
            };
            let actor = inner
                .user_actors
                .get(&user_id)
                .cloned()
                .unwrap_or_else(system_actor);
            let mut events = Vec::new();

            for project_id in project_ids {
                let Some(not_subscribed) = inner
                    .connections
                    .get_mut(&connection_id)
                    .map(|connection| !connection.subscribed_projects.remove(project_id))
                else {
                    continue;
                };
                if not_subscribed {
                    continue;
                }
                if let Some(subscribers) = inner.project_subscribers.get_mut(project_id) {
                    subscribers.remove(&connection_id);
                    if subscribers.is_empty() {
                        inner.project_subscribers.remove(project_id);
                    }
                }

                if let Some(removed_presence) =
                    decrement_presence(&mut inner.project_presence, *project_id, user_id)
                {
                    events.push(build_presence_event(
                        *project_id,
                        actor.clone(),
                        "project",
                        Some(*project_id),
                        "offline",
                        removed_presence.route,
                        None,
                    ));
                }
            }

            let targets = collect_project_targets(
                &inner,
                events
                    .iter()
                    .filter_map(|event| event.project_id)
                    .collect::<Vec<_>>(),
            );
            (events, targets)
        };

        send_events(targets, events);
    }

    pub async fn broadcast_project(
        &self,
        project_id: Uuid,
        event: RealtimeEvent,
        exclude_user_id: Option<Uuid>,
    ) {
        let targets = {
            let inner = self.inner.read().await;
            collect_project_targets_filtered(&inner, project_id, exclude_user_id)
        };
        send_events(targets, vec![event]);
    }

    pub async fn send_to_user(&self, user_id: Uuid, event: RealtimeEvent) {
        let targets = {
            let inner = self.inner.read().await;
            inner
                .user_connections
                .get(&user_id)
                .into_iter()
                .flat_map(|ids| ids.iter())
                .filter_map(|id| inner.connections.get(id))
                .map(|connection| connection.sender.clone())
                .collect::<Vec<_>>()
        };
        send_events(targets, vec![event]);
    }

    pub async fn send_error(&self, connection_id: Uuid, message: impl Into<String>) {
        let sender = {
            let inner = self.inner.read().await;
            inner
                .connections
                .get(&connection_id)
                .map(|connection| connection.sender.clone())
        };
        if let Some(sender) = sender {
            let _ = sender.send(RealtimeOutbound::Error(RealtimeErrorMessage {
                error: "bad_request".to_string(),
                message: message.into(),
            }));
        }
    }

    pub async fn send_ping_to_connection(&self, connection_id: Uuid) {
        let sender = {
            let inner = self.inner.read().await;
            inner
                .connections
                .get(&connection_id)
                .map(|connection| connection.sender.clone())
        };
        if let Some(sender) = sender {
            let _ = sender.send(RealtimeOutbound::Ping);
        }
    }

    #[allow(dead_code)]
    pub async fn send_ping_to_user(&self, user_id: Uuid) {
        let targets = {
            let inner = self.inner.read().await;
            inner
                .user_connections
                .get(&user_id)
                .into_iter()
                .flat_map(|ids| ids.iter())
                .filter_map(|id| inner.connections.get(id))
                .map(|connection| connection.sender.clone())
                .collect::<Vec<_>>()
        };
        for sender in targets {
            let _ = sender.send(RealtimeOutbound::Ping);
        }
    }

    pub async fn force_unsubscribe_user_from_project(&self, user_id: Uuid, project_id: Uuid) {
        let (events, targets, direct_targets) = {
            let mut inner = self.inner.write().await;
            let actor = inner
                .user_actors
                .get(&user_id)
                .cloned()
                .unwrap_or_else(system_actor);
            let mut removed_any = false;
            let mut direct_targets = Vec::new();
            if let Some(connection_ids) = inner.user_connections.get(&user_id).cloned() {
                for connection_id in connection_ids {
                    if let Some(connection) = inner.connections.get_mut(&connection_id) {
                        let removed = connection.subscribed_projects.remove(&project_id);
                        removed_any |= removed;
                        if removed {
                            direct_targets.push(connection.sender.clone());
                        }
                    }
                    if let Some(subscribers) = inner.project_subscribers.get_mut(&project_id) {
                        subscribers.remove(&connection_id);
                        if subscribers.is_empty() {
                            inner.project_subscribers.remove(&project_id);
                        }
                    }
                }
            }

            let mut events = Vec::new();
            if let Some(removed_presence) = removed_any
                .then(|| decrement_presence_all(&mut inner.project_presence, project_id, user_id))
                .flatten()
            {
                events.push(build_presence_event(
                    project_id,
                    actor,
                    "project",
                    Some(project_id),
                    "offline",
                    removed_presence.route,
                    None,
                ));
            }
            let targets = collect_project_targets(&inner, vec![project_id]);
            (events, targets, direct_targets)
        };

        send_events(targets, events);
        let closed_event = build_presence_event(
            project_id,
            system_actor(),
            "project",
            Some(project_id),
            "closed",
            None,
            None,
        );
        for sender in direct_targets {
            let _ = sender.send(RealtimeOutbound::Event(closed_event.clone()));
        }
    }

    pub async fn force_unsubscribe_project(&self, project_id: Uuid) {
        let targets = {
            let mut inner = self.inner.write().await;
            let subscriber_ids = inner
                .project_subscribers
                .remove(&project_id)
                .unwrap_or_default();
            for connection_id in &subscriber_ids {
                if let Some(connection) = inner.connections.get_mut(connection_id) {
                    connection.subscribed_projects.remove(&project_id);
                }
            }
            inner.project_presence.remove(&project_id);
            subscriber_ids
                .into_iter()
                .filter_map(|connection_id| inner.connections.get(&connection_id))
                .map(|connection| connection.sender.clone())
                .collect::<Vec<_>>()
        };

        let event = build_presence_event(
            project_id,
            system_actor(),
            "project",
            Some(project_id),
            "closed",
            None,
            None,
        );
        send_events(targets, vec![event]);
    }
}

pub async fn start_realtime_dispatcher(
    mut rx: broadcast::Receiver<Value>,
    pool: SqlitePool,
    hub: RealtimeHub,
) {
    tracing::info!("Realtime dispatcher started");
    loop {
        let event = match rx.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(dropped = n, "realtime dispatcher lagged, dropped events");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        };
        match extract_project_change(&pool, &event).await {
            Ok(Some(change)) => {
                if let Err(error) = dispatch_project_change(&pool, &hub, change).await {
                    tracing::error!("Failed to dispatch realtime event: {}", error);
                }
            }
            Ok(None) => {}
            Err(error) => tracing::error!("Failed to parse realtime event: {}", error),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectChange {
    pub event_id: Uuid,
    pub project_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor: RealtimeActor,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub change_type: String,
    pub occurred_at: DateTime<Utc>,
}

pub fn build_presence_event(
    project_id: Uuid,
    actor: RealtimeActor,
    entity_type: &str,
    entity_id: Option<Uuid>,
    change_type: &str,
    route: Option<String>,
    project_snapshot: Option<Vec<RealtimePresenceUser>>,
) -> RealtimeEvent {
    let occurred_at = Utc::now();
    RealtimeEvent {
        family: RealtimeEventFamily::Presence,
        project_id: Some(project_id),
        event_id: Uuid::new_v4(),
        metadata: build_presence_metadata(
            &actor,
            change_type,
            occurred_at,
            route,
            project_snapshot,
        ),
        actor,
        entity_type: entity_type.to_string(),
        entity_id,
        change_type: change_type.to_string(),
        occurred_at,
        unread_count: None,
    }
}

pub fn build_data_changed_event(change: &ProjectChange) -> RealtimeEvent {
    RealtimeEvent {
        family: RealtimeEventFamily::DataChanged,
        project_id: Some(change.project_id),
        event_id: change.event_id,
        actor: change.actor.clone(),
        entity_type: change.entity_type.clone(),
        entity_id: change.entity_id,
        change_type: change.change_type.clone(),
        occurred_at: change.occurred_at,
        unread_count: None,
        metadata: None,
    }
}

pub fn build_notification_event(
    change: &ProjectChange,
    unread_count: i64,
    notification_id: Option<Uuid>,
) -> RealtimeEvent {
    RealtimeEvent {
        family: RealtimeEventFamily::Notification,
        project_id: Some(change.project_id),
        event_id: change.event_id,
        actor: change.actor.clone(),
        entity_type: change.entity_type.clone(),
        entity_id: notification_id.or(change.entity_id),
        change_type: change.change_type.clone(),
        occurred_at: change.occurred_at,
        unread_count: Some(unread_count),
        metadata: None,
    }
}

fn build_presence_metadata(
    actor: &RealtimeActor,
    change_type: &str,
    occurred_at: DateTime<Utc>,
    route: Option<String>,
    project_snapshot: Option<Vec<RealtimePresenceUser>>,
) -> Option<RealtimeEventMetadata> {
    let status = match change_type {
        "snapshot" | "online" | "updated" => PresenceStatus::Online,
        "offline" => PresenceStatus::Offline,
        _ => return None,
    };

    Some(RealtimeEventMetadata::Presence(RealtimePresenceMetadata {
        user_id: actor.id,
        status,
        last_seen_at: occurred_at,
        route,
        project_snapshot,
    }))
}

pub fn notification_severity(change_type: &str) -> NotificationSeverity {
    match change_type {
        "deleted" | "removed" => NotificationSeverity::Critical,
        "created" | "updated" => NotificationSeverity::Important,
        _ => NotificationSeverity::Noise,
    }
}

pub fn notification_route(
    project_id: Option<Uuid>,
    entity_type: &str,
    entity_id: Option<Uuid>,
) -> Option<String> {
    let project_id = project_id?;
    match entity_type {
        "task" => entity_id.map(|entity_id| format!("/projects/{project_id}/tasks/{entity_id}")),
        "project_membership" => Some(format!("/projects/{project_id}/members")),
        "project" => Some(format!("/projects/{project_id}")),
        _ => Some(format!("/projects/{project_id}")),
    }
}

pub fn notification_title(
    entity_type: &str,
    change_type: &str,
    entity_title: Option<&str>,
) -> String {
    let entity_label = entity_title
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" \"{}\"", value.trim()))
        .unwrap_or_default();
    format!(
        "{} {}{}",
        entity_type_label(entity_type),
        action_label(change_type),
        entity_label
    )
}

pub fn notification_message(
    actor_name: &str,
    entity_type: &str,
    change_type: &str,
    entity_title: Option<&str>,
    project_name: Option<&str>,
) -> String {
    let mut message = format!(
        "{} {} {}",
        actor_name,
        action_label(change_type).to_lowercase(),
        entity_type_label(entity_type).to_lowercase()
    );
    if let Some(entity_title) = entity_title.filter(|value| !value.trim().is_empty()) {
        message.push_str(&format!(" \"{}\"", entity_title.trim()));
    }
    if let Some(project_name) = project_name.filter(|value| !value.trim().is_empty()) {
        message.push_str(&format!(" in {}", project_name.trim()));
    }
    message.push('.');
    message
}

fn entity_type_label(entity_type: &str) -> &'static str {
    match entity_type {
        "project" => "Project",
        "task" => "Task",
        "project_membership" => "Project membership",
        "dependency" => "Dependency",
        "work_log" => "Work log",
        "progress" => "Progress",
        _ => "Activity",
    }
}

fn action_label(change_type: &str) -> &'static str {
    match change_type {
        "created" => "Created",
        "updated" => "Updated",
        "deleted" => "Deleted",
        "removed" => "Removed",
        "restored" => "Restored",
        _ => "Updated",
    }
}

pub async fn user_can_view_project(
    pool: &SqlitePool,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<bool> {
    let match_project = uuid_sql::match_uuid_clause("p.id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let sql = format!(
        "SELECT CASE WHEN {} THEN 1 ELSE 0 END AS is_owner
         FROM projects p
         WHERE {} AND p.deleted_at IS NULL
         LIMIT 1",
        match_owner, match_project
    );

    let exists_and_owned: Option<i64> = sqlx::query_scalar(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(pool)
        .await?;

    let Some(is_owner) = exists_and_owned else {
        return Ok(false);
    };
    if is_owner == 1 {
        return Ok(true);
    }

    let principal = Principal::load(user_id, pool)
        .await
        .map_err(|error| AppError::internal(format!("failed to load principal: {}", error)))?;
    let evaluator = DefaultPolicyEvaluator::new();
    Ok(evaluator
        .can(
            &principal,
            "project.view",
            &ResourceContext::new().with_project(project_id),
        )
        .await)
}

pub async fn fetch_actor_info(pool: &SqlitePool, user_id: Uuid) -> AppResult<RealtimeActor> {
    let match_user = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "SELECT name FROM users WHERE {} AND deleted_at IS NULL",
        match_user
    );
    let name: Option<String> = sqlx::query_scalar(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(RealtimeActor {
        id: user_id,
        name: name.unwrap_or_else(|| "unknown".to_string()),
    })
}

pub fn system_actor() -> RealtimeActor {
    RealtimeActor {
        id: Uuid::nil(),
        name: "system".to_string(),
    }
}

#[allow(dead_code)]
pub fn to_notification_actor(actor: &RealtimeActor) -> NotificationActor {
    NotificationActor {
        id: actor.id,
        name: actor.name.clone(),
    }
}

pub async fn count_visible_unread_notifications(
    pool: &SqlitePool,
    user_id: Uuid,
) -> AppResult<i64> {
    let match_recipient = uuid_sql::match_uuid_clause("n.user_id");
    let project_case = uuid_sql::case_uuid("n.project_id");
    let sql = format!(
        "SELECT {}
         FROM notifications n
         WHERE {} AND n.deleted_at IS NULL AND n.read_at IS NULL",
        project_case, match_recipient
    );

    let rows = sqlx::query(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_all(pool)
        .await?;

    let mut visible_count = 0_i64;
    let mut project_cache: HashMap<Uuid, bool> = HashMap::new();

    for row in rows {
        let project_id: Option<String> = row.try_get("project_id")?;
        let is_visible = match project_id {
            None => true,
            Some(project_id) => {
                let project_id = Uuid::parse_str(&project_id).map_err(|error| {
                    AppError::internal(format!("invalid notification project id: {}", error))
                })?;
                if let Some(is_visible) = project_cache.get(&project_id) {
                    *is_visible
                } else {
                    let is_visible = user_can_view_project(pool, user_id, project_id).await?;
                    project_cache.insert(project_id, is_visible);
                    is_visible
                }
            }
        };

        if is_visible {
            visible_count += 1;
        }
    }

    Ok(visible_count)
}

pub async fn create_direct_notification(
    pool: &SqlitePool,
    recipient_user_id: Uuid,
    project_id: Option<Uuid>,
    event_id: Uuid,
    actor: RealtimeActor,
    entity_type: &str,
    entity_id: Option<Uuid>,
    change_type: &str,
    occurred_at: DateTime<Utc>,
) -> AppResult<(Uuid, i64)> {
    let notification_id = Uuid::new_v4();
    let match_user = uuid_sql::match_uuid_clause("id");
    let project_match = uuid_sql::match_uuid_clause("id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let project_id_text = project_id.map(|id| id.to_string());
    let actor_id_text = (actor.id != Uuid::nil()).then(|| actor.id.to_string());
    let sql = format!(
        "INSERT OR IGNORE INTO notifications (
            id,
            user_id,
            project_id,
            event_id,
            actor_id,
            actor_name,
            entity_type,
            entity_id,
            change_type,
            occurred_at,
            created_at
         ) VALUES (
            ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
            CASE
                WHEN ? IS NULL THEN NULL
                ELSE (SELECT id FROM projects WHERE {} LIMIT 1)
            END,
            ?,
            CASE
                WHEN ? IS NULL THEN NULL
                ELSE (SELECT id FROM users WHERE {} LIMIT 1)
            END,
            ?, ?, ?, ?, ?, ?
        )",
        match_user, project_match, actor_match
    );

    let inserted = sqlx::query(&sql)
        .bind(notification_id.to_string())
        .bind(recipient_user_id.to_string())
        .bind(recipient_user_id.to_string())
        .bind(project_id_text.clone())
        .bind(project_id_text.clone())
        .bind(project_id_text.clone())
        .bind(event_id.to_string())
        .bind(actor_id_text.clone())
        .bind(actor_id_text.clone())
        .bind(actor_id_text.clone())
        .bind(&actor.name)
        .bind(entity_type)
        .bind(entity_id.map(|id| id.to_string()))
        .bind(change_type)
        .bind(occurred_at)
        .bind(Utc::now())
        .execute(pool)
        .await?;

    let notification_id = if inserted.rows_affected() == 0 {
        let notification_match = uuid_sql::match_uuid_clause("user_id");
        let sql = format!(
            "SELECT {} FROM notifications WHERE {} AND event_id = ? LIMIT 1",
            uuid_sql::case_uuid("id"),
            notification_match
        );
        let existing_id: Option<String> = sqlx::query_scalar(&sql)
            .bind(recipient_user_id.to_string())
            .bind(recipient_user_id.to_string())
            .bind(event_id.to_string())
            .fetch_optional(pool)
            .await?;
        existing_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .unwrap_or(notification_id)
    } else {
        notification_id
    };

    let unread_count = count_visible_unread_notifications(pool, recipient_user_id).await?;
    Ok((notification_id, unread_count))
}

pub async fn notify_project_members(
    pool: &SqlitePool,
    hub: &RealtimeHub,
    change: &ProjectChange,
    include_deleted_project: bool,
) -> AppResult<()> {
    let recipients =
        visible_project_recipient_ids(pool, change.project_id, include_deleted_project).await?;
    for recipient in recipients {
        if Some(recipient) == change.actor_id {
            continue;
        }
        let (notification_id, unread_count) = create_direct_notification(
            pool,
            recipient,
            Some(change.project_id),
            change.event_id,
            change.actor.clone(),
            &change.entity_type,
            change.entity_id,
            &change.change_type,
            change.occurred_at,
        )
        .await?;
        hub.send_to_user(
            recipient,
            build_notification_event(change, unread_count, Some(notification_id)),
        )
        .await;
    }
    Ok(())
}

pub async fn emit_membership_refresh_event(
    hub: &RealtimeHub,
    target_user_id: Uuid,
    actor: RealtimeActor,
    project_id: Uuid,
    change_type: &str,
) {
    hub.send_to_user(
        target_user_id,
        RealtimeEvent {
            family: RealtimeEventFamily::DataChanged,
            project_id: Some(project_id),
            event_id: Uuid::new_v4(),
            actor,
            entity_type: "project_membership".to_string(),
            entity_id: Some(target_user_id),
            change_type: change_type.to_string(),
            occurred_at: Utc::now(),
            unread_count: None,
            metadata: Some(RealtimeEventMetadata::Invalidate(
                RealtimeInvalidateMetadata {
                    invalidate: vec!["projects".to_string(), "users_me_projects".to_string()],
                },
            )),
        },
    )
    .await;
}

pub async fn dispatch_project_change(
    pool: &SqlitePool,
    hub: &RealtimeHub,
    change: ProjectChange,
) -> AppResult<()> {
    let include_deleted_project =
        change.entity_type == "project" && change.change_type == "deleted";
    notify_project_members(pool, hub, &change, include_deleted_project).await?;
    hub.broadcast_project(
        change.project_id,
        build_data_changed_event(&change),
        change.actor_id,
    )
    .await;

    if include_deleted_project {
        hub.force_unsubscribe_project(change.project_id).await;
    }
    Ok(())
}

pub async fn extract_project_change(
    pool: &SqlitePool,
    event: &Value,
) -> AppResult<Option<ProjectChange>> {
    let event_id = event
        .get("id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::new_v4);
    let name = event
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let mut parts = name.split('.');
    let entity_type = parts.next().unwrap_or_default().to_string();
    let change_type = parts.next().unwrap_or_default().to_string();
    if entity_type.is_empty() || change_type.is_empty() {
        return Ok(None);
    }

    let actor_id = event
        .get("actor_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok());
    let actor = match actor_id {
        Some(user_id) => fetch_actor_info(pool, user_id).await?,
        None => system_actor(),
    };
    let entity_id = event
        .get("subject_id")
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok());
    let occurred_at = event
        .get("occurred_at")
        .and_then(|value| value.as_str())
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let current = event
        .get("payload")
        .and_then(|payload| payload.get("new").or_else(|| payload.get("current")))
        .unwrap_or(&Value::Null);
    let old = event
        .get("payload")
        .and_then(|payload| payload.get("old"))
        .unwrap_or(&Value::Null);
    let project_id = match entity_type.as_str() {
        "project" => entity_id,
        "dependency" => extract_dependency_project_id(pool, current, old).await?,
        _ => extract_uuid_field(current, "project_id")
            .or_else(|| extract_uuid_field(old, "project_id")),
    };

    let Some(project_id) = project_id else {
        return Ok(None);
    };

    Ok(Some(ProjectChange {
        event_id,
        project_id,
        actor_id,
        actor,
        entity_type,
        entity_id,
        change_type,
        occurred_at,
    }))
}

async fn extract_dependency_project_id(
    pool: &SqlitePool,
    current: &Value,
    old: &Value,
) -> AppResult<Option<Uuid>> {
    let task_id = extract_uuid_field(current, "source_task_id")
        .or_else(|| extract_uuid_field(current, "target_task_id"))
        .or_else(|| extract_uuid_field(old, "source_task_id"))
        .or_else(|| extract_uuid_field(old, "target_task_id"));
    let Some(task_id) = task_id else {
        return Ok(None);
    };

    let match_task = uuid_sql::match_uuid_clause("t.id");
    let project_case = uuid_sql::case_uuid("t.project_id");
    let sql = format!(
        "SELECT {} FROM tasks t WHERE {} AND t.deleted_at IS NULL LIMIT 1",
        project_case, match_task
    );
    let project_id: Option<String> = sqlx::query_scalar(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_optional(pool)
        .await?;
    Ok(project_id.and_then(|value| Uuid::parse_str(&value).ok()))
}

fn extract_uuid_field(value: &Value, field: &str) -> Option<Uuid> {
    value
        .get(field)
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
}

async fn visible_project_recipient_ids(
    pool: &SqlitePool,
    project_id: Uuid,
    include_deleted_project: bool,
) -> AppResult<Vec<Uuid>> {
    let project_match = uuid_sql::match_uuid_clause("p.id");
    let user_case = uuid_sql::case_uuid("visible.user_id");
    let project_deleted_filter = if include_deleted_project {
        ""
    } else {
        " AND p.deleted_at IS NULL"
    };
    let sql = format!(
        "SELECT DISTINCT {user_case}
         FROM (
            SELECT p.user_id AS user_id
            FROM projects p
            WHERE {project_match}{project_deleted_filter}
            UNION
            SELECT pm.user_id AS user_id
            FROM project_members pm
            WHERE pm.project_id = (SELECT id FROM projects p WHERE {project_match} LIMIT 1)
              AND pm.deleted_at IS NULL
         ) visible
         INNER JOIN users u ON u.id = visible.user_id
         WHERE u.deleted_at IS NULL",
        user_case = user_case,
        project_match = project_match,
        project_deleted_filter = project_deleted_filter,
    );

    let rows = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(pool)
        .await?;

    let mut recipients = Vec::with_capacity(rows.len());
    for row in rows {
        let user_id: String = row.try_get("user_id")?;
        if let Ok(user_id) = Uuid::parse_str(&user_id) {
            if user_can_view_project(pool, user_id, project_id).await? {
                recipients.push(user_id);
            }
        }
    }
    Ok(recipients)
}

fn increment_presence(
    presence: &mut HashMap<Uuid, HashMap<Uuid, PresenceState>>,
    project_id: Uuid,
    user_id: Uuid,
    route: Option<String>,
) -> PresenceChange {
    let now = Utc::now();
    let project_presence = presence.entry(project_id).or_default();
    let entry = project_presence
        .entry(user_id)
        .or_insert_with(|| PresenceState {
            connections: 0,
            route: route.clone(),
            last_seen_at: now,
        });

    let first_connection = entry.connections == 0;
    let previous_route = entry.route.clone();
    entry.connections += 1;
    entry.last_seen_at = now;
    if route.is_some() {
        entry.route = route.clone();
    }

    PresenceChange {
        first_connection,
        route_changed: !first_connection && route.is_some() && previous_route != entry.route,
        route: entry.route.clone(),
    }
}

fn refresh_presence(
    presence: &mut HashMap<Uuid, HashMap<Uuid, PresenceState>>,
    project_id: Uuid,
    user_id: Uuid,
    route: Option<String>,
) -> PresenceChange {
    let now = Utc::now();
    let project_presence = presence.entry(project_id).or_default();
    let entry = project_presence
        .entry(user_id)
        .or_insert_with(|| PresenceState {
            connections: 0,
            route: route.clone(),
            last_seen_at: now,
        });
    let previous_route = entry.route.clone();
    entry.last_seen_at = now;
    if route.is_some() {
        entry.route = route.clone();
    }

    PresenceChange {
        first_connection: false,
        route_changed: route.is_some() && previous_route != entry.route,
        route: entry.route.clone(),
    }
}

fn decrement_presence(
    presence: &mut HashMap<Uuid, HashMap<Uuid, PresenceState>>,
    project_id: Uuid,
    user_id: Uuid,
) -> Option<PresenceState> {
    let Some(project_presence) = presence.get_mut(&project_id) else {
        return None;
    };
    let Some(entry) = project_presence.get_mut(&user_id) else {
        return None;
    };
    if entry.connections > 1 {
        entry.connections -= 1;
        entry.last_seen_at = Utc::now();
        return None;
    }
    let removed = project_presence.remove(&user_id);
    if project_presence.is_empty() {
        presence.remove(&project_id);
    }
    removed
}

fn decrement_presence_all(
    presence: &mut HashMap<Uuid, HashMap<Uuid, PresenceState>>,
    project_id: Uuid,
    user_id: Uuid,
) -> Option<PresenceState> {
    let Some(project_presence) = presence.get_mut(&project_id) else {
        return None;
    };
    let removed = project_presence.remove(&user_id);
    if project_presence.is_empty() {
        presence.remove(&project_id);
    }
    removed
}

fn snapshot_online_users(inner: &HubState, project_id: Uuid) -> Vec<RealtimePresenceUser> {
    inner
        .project_presence
        .get(&project_id)
        .into_iter()
        .flat_map(|users| users.iter())
        .filter_map(|(user_id, presence)| {
            inner
                .user_actors
                .get(user_id)
                .map(|actor| RealtimePresenceUser {
                    user_id: actor.id,
                    name: actor.name.clone(),
                    status: PresenceStatus::Online,
                    last_seen_at: presence.last_seen_at,
                    route: presence.route.clone(),
                })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct PresenceChange {
    first_connection: bool,
    route_changed: bool,
    route: Option<String>,
}

#[allow(dead_code)]
fn current_presence(
    presence: &HashMap<Uuid, HashMap<Uuid, PresenceState>>,
    project_id: Uuid,
    user_id: Uuid,
) -> Option<PresenceState> {
    presence
        .get(&project_id)
        .and_then(|users| users.get(&user_id))
        .cloned()
}

fn collect_project_targets(
    inner: &HubState,
    project_ids: Vec<Uuid>,
) -> Vec<mpsc::UnboundedSender<RealtimeOutbound>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for project_id in project_ids {
        if let Some(subscribers) = inner.project_subscribers.get(&project_id) {
            for connection_id in subscribers {
                if seen.insert(*connection_id) {
                    if let Some(connection) = inner.connections.get(connection_id) {
                        out.push(connection.sender.clone());
                    }
                }
            }
        }
    }
    out
}

fn collect_project_targets_filtered(
    inner: &HubState,
    project_id: Uuid,
    exclude_user_id: Option<Uuid>,
) -> Vec<mpsc::UnboundedSender<RealtimeOutbound>> {
    inner
        .project_subscribers
        .get(&project_id)
        .into_iter()
        .flat_map(|subscribers| subscribers.iter())
        .filter_map(|connection_id| inner.connections.get(connection_id))
        .filter(|connection| exclude_user_id != Some(connection.user_id))
        .map(|connection| connection.sender.clone())
        .collect()
}

fn send_events(targets: Vec<mpsc::UnboundedSender<RealtimeOutbound>>, events: Vec<RealtimeEvent>) {
    if events.is_empty() {
        return;
    }
    for sender in targets {
        for event in &events {
            let _ = sender.send(RealtimeOutbound::Event(event.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribe_and_disconnect_emit_presence_events() {
        let hub = RealtimeHub::new();
        let actor = RealtimeActor {
            id: Uuid::new_v4(),
            name: "Alice".to_string(),
        };
        let (connection_id, mut rx) = hub.register_connection(actor.id, actor.clone()).await;
        let project_id = Uuid::new_v4();

        hub.subscribe_projects(connection_id, &[project_id], None)
            .await;

        let snapshot = rx.recv().await.expect("snapshot event");
        match snapshot {
            RealtimeOutbound::Event(event) => {
                assert_eq!(event.family, RealtimeEventFamily::Presence);
                assert_eq!(event.change_type, "snapshot");
                assert_eq!(event.project_id, Some(project_id));
            }
            _ => panic!("expected snapshot event"),
        }

        let online = rx.recv().await.expect("presence event");
        match online {
            RealtimeOutbound::Event(event) => {
                assert_eq!(event.family, RealtimeEventFamily::Presence);
                assert_eq!(event.change_type, "online");
                assert_eq!(event.project_id, Some(project_id));
            }
            _ => panic!("expected presence event"),
        }

        hub.unregister_connection(connection_id).await;
    }
}
