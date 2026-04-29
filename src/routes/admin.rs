use axum::extract::{Path, State};
use axum::Json;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::app::AppState;
use crate::db::uuid_sql;
use crate::errors::AppError;
use crate::errors::AppResult;
use crate::jwt::AuthUser;
use crate::models::admin::TaskHistoryEntry;

/// Get full task change history (admin only)
///
/// Returns every `task.*` event recorded in the audit log for a task,
/// with actor names resolved from the users table. Requires the
/// `task.history` permission (assigned to `admin` and `super_admin` roles only).
///
/// Events are returned in reverse-chronological order.
///
/// **Changes object** structure varies by event type:
/// - `task.created` — full initial field set
/// - `task.updated` — `{ "field": [old_value, new_value] }` pairs
/// - `task.deleted` — `{ "deleted_at": "..." }`
#[utoipa::path(
    get,
    path = "/admin/tasks/{task_id}/history",
    tag = "Admin",
    params(
        ("task_id" = Uuid, Path, description = "Task id")
    ),
    responses(
        (status = 200, description = "Task change history", body = Vec<TaskHistoryEntry>),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Requires task.history permission (admin only)"),
        (status = 404, description = "Task not found"),
    ),
    security(("bearerAuth" = []))
)]
pub async fn list_task_history(
    State(state): State<AppState>,
    _auth: AuthUser, // identity available if needed; authz middleware already verified task.history
    Path(task_id): Path<Uuid>,
) -> AppResult<Json<Vec<TaskHistoryEntry>>> {
    // Verify the task exists (soft-delete aware).
    let match_task = uuid_sql::match_uuid_clause("id");
    let task_check_sql = format!(
        "SELECT COUNT(1) FROM tasks WHERE {} AND deleted_at IS NULL",
        match_task
    );
    let count: i64 = sqlx::query_scalar(&task_check_sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_one(&state.pool)
        .await?;

    if count == 0 {
        return Err(AppError::not_found("task not found"));
    }

    // Fetch all activity_log entries for this task.
    // The composite index on (subject_id, event_name, occurred_at DESC) makes this fast.
    let match_subject = uuid_sql::match_uuid_clause("subject_id");
    let history_sql = format!(
        "SELECT id, event_name, actor_id, occurred_at, properties \
         FROM activity_log \
         WHERE {} AND event_name LIKE 'task.%' \
         ORDER BY occurred_at DESC",
        match_subject
    );

    let rows = sqlx::query(&history_sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    // Collect unique actor_ids for batch name resolution.
    let actor_id_strings: Vec<String> = rows
        .iter()
        .filter_map(|r| r.try_get::<Option<String>, _>("actor_id").ok().flatten())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Resolve actor names in a single query (two binds per id for blob/text UUID handling).
    let mut actor_names: HashMap<String, String> = HashMap::new();
    if !actor_id_strings.is_empty() {
        let id_case = uuid_sql::case_uuid("id");
        let where_parts: Vec<String> = actor_id_strings
            .iter()
            .map(|_| format!("({})", uuid_sql::match_uuid_clause("id")))
            .collect();
        let user_sql = format!(
            "SELECT {}, name FROM users WHERE ({}) AND deleted_at IS NULL",
            id_case,
            where_parts.join(" OR ")
        );
        let mut query = sqlx::query(&user_sql);
        for id in &actor_id_strings {
            query = query.bind(id.as_str()).bind(id.as_str());
        }
        for row in query.fetch_all(&state.pool).await? {
            let id: String = row.get("id");
            let name: String = row.get("name");
            actor_names.insert(id, name);
        }
    }

    // Build response entries.
    let entries: Vec<TaskHistoryEntry> = rows
        .iter()
        .map(|row| {
            let id: String = row.try_get("id").unwrap_or_default();
            let event: String = row.try_get("event_name").unwrap_or_default();

            let actor_id_str: Option<String> =
                row.try_get("actor_id").ok().flatten();
            let actor_id = actor_id_str
                .as_deref()
                .and_then(|s| Uuid::parse_str(s).ok());
            let actor_name = actor_id_str
                .as_deref()
                .and_then(|s| actor_names.get(s))
                .cloned();

            let occurred_at: chrono::DateTime<chrono::Utc> =
                row.try_get("occurred_at").unwrap_or_else(|_| chrono::Utc::now());

            let changes = row
                .try_get::<Option<String>, _>("properties")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .unwrap_or(serde_json::Value::Null);

            TaskHistoryEntry {
                id,
                event,
                actor_id,
                actor_name,
                occurred_at,
                changes,
            }
        })
        .collect();

    Ok(Json(entries))
}
