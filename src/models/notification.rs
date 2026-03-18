use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Critical,
    Important,
    Noise,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct NotificationActor {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Notification {
    pub id: Uuid,
    pub event_id: Uuid,
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub actor: NotificationActor,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub change_type: String,
    pub title: String,
    pub message: String,
    pub route: Option<String>,
    pub severity: NotificationSeverity,
    pub occurred_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub unread: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkNotificationsReadRequest {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationUnreadCountResponse {
    pub unread_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationsReadResponse {
    pub updated: usize,
    pub unread_count: i64,
}
