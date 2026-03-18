use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeEventFamily {
    Notification,
    Presence,
    DataChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct RealtimeActor {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresenceStatus {
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct RealtimePresenceUser {
    pub user_id: Uuid,
    pub name: String,
    pub status: PresenceStatus,
    pub last_seen_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct RealtimePresenceMetadata {
    pub user_id: Uuid,
    pub status: PresenceStatus,
    pub last_seen_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_snapshot: Option<Vec<RealtimePresenceUser>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct RealtimeNotificationCounterMetadata {
    pub updated: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct RealtimeInvalidateMetadata {
    pub invalidate: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(untagged)]
pub enum RealtimeEventMetadata {
    Presence(RealtimePresenceMetadata),
    NotificationCounter(RealtimeNotificationCounterMetadata),
    Invalidate(RealtimeInvalidateMetadata),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RealtimeEvent {
    pub family: RealtimeEventFamily,
    pub project_id: Option<Uuid>,
    pub event_id: Uuid,
    pub actor: RealtimeActor,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub change_type: String,
    pub occurred_at: DateTime<Utc>,
    pub unread_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RealtimeEventMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RealtimeErrorMessage {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RealtimeClientCommand {
    Subscribe {
        project_ids: Vec<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        route: Option<String>,
    },
    Unsubscribe {
        project_ids: Vec<Uuid>,
    },
    Ping,
}
