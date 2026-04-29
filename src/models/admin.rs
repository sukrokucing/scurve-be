use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A single entry in a task's automatic change history.
#[derive(Debug, Serialize, ToSchema)]
pub struct TaskHistoryEntry {
    /// Activity log entry id.
    pub id: String,
    /// Domain event name, e.g. `"task.created"`, `"task.updated"`, `"task.deleted"`.
    pub event: String,
    /// UUID of the user who triggered the change.
    pub actor_id: Option<Uuid>,
    /// Display name of the actor (resolved from the users table).
    pub actor_name: Option<String>,
    /// When the change occurred (UTC).
    pub occurred_at: DateTime<Utc>,
    /// Raw event properties — structure varies by event type.
    /// For `task.updated` this typically contains `{ "field": [old, new] }` pairs.
    #[schema(value_type = Object)]
    pub changes: serde_json::Value,
}
