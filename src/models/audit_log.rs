use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for listing audit logs
#[derive(Debug, Deserialize, IntoParams)]
pub struct AuditLogFilter {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: i64,
    /// Items per page (max 100)
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Filter by action/event name (e.g., "role.assign")
    pub action: Option<String>,
    /// Filter by target user ID (subject_id)
    pub user_id: Option<Uuid>,
    /// Filter by actor ID (who performed the action)
    pub actor_id: Option<Uuid>,
    /// Filter entries from this timestamp
    pub from: Option<DateTime<Utc>>,
    /// Filter entries until this timestamp
    pub to: Option<DateTime<Utc>>,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    25
}

/// A single audit log entry
#[derive(Debug, Serialize, ToSchema)]
pub struct AuditLogEntry {
    pub id: String,
    /// Action/event name (e.g., "role.assign", "permission.grant")
    pub action: String,
    /// ID of the user who performed the action
    pub actor_id: Option<Uuid>,
    /// Display name of the actor
    pub actor_name: Option<String>,
    /// ID of the target user (if applicable)
    pub target_user_id: Option<Uuid>,
    /// Display name of the target user
    pub target_user_name: Option<String>,
    /// Action-specific metadata (role_id, permission_id, etc.)
    #[schema(value_type = Object)]
    pub details: Value,
    /// When the action occurred
    pub created_at: DateTime<Utc>,
}

/// Paginated response for audit logs
#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedAuditLogs {
    pub items: Vec<AuditLogEntry>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}
