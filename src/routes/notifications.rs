use std::collections::{HashMap, HashSet};

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use sqlx::Row;
use uuid::Uuid;

use crate::app::AppState;
use crate::db::uuid_sql;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::notification::{
    MarkNotificationsReadRequest, Notification, NotificationActor, NotificationUnreadCountResponse,
    NotificationsReadResponse,
};
use crate::models::realtime::{
    RealtimeEvent, RealtimeEventFamily, RealtimeEventMetadata, RealtimeNotificationCounterMetadata,
};
use crate::realtime::{
    count_visible_unread_notifications, fetch_actor_info, notification_message, notification_route,
    notification_severity, notification_title, system_actor, user_can_view_project,
};

fn parse_db_datetime(value: &str) -> AppResult<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
    }
    Err(AppError::internal(format!("invalid datetime: {}", value)))
}

#[utoipa::path(
    get,
    path = "/notifications",
    tag = "Notifications",
    responses((status = 200, description = "Visible notifications for the current user", body = [Notification])),
    security(("bearerAuth" = []))
)]
pub async fn list_notifications(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Notification>>> {
    Ok(Json(
        load_visible_notifications(&state, auth.user_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/notifications/unread-count",
    tag = "Notifications",
    responses((status = 200, description = "Unread notification count", body = NotificationUnreadCountResponse)),
    security(("bearerAuth" = []))
)]
pub async fn get_unread_notification_count(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<NotificationUnreadCountResponse>> {
    let unread_count = count_visible_unread_notifications(&state.pool, auth.user_id).await?;
    Ok(Json(NotificationUnreadCountResponse { unread_count }))
}

#[utoipa::path(
    post,
    path = "/notifications/read",
    tag = "Notifications",
    request_body = MarkNotificationsReadRequest,
    responses((status = 200, description = "Notifications marked as read", body = NotificationsReadResponse)),
    security(("bearerAuth" = []))
)]
pub async fn mark_notifications_read(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<MarkNotificationsReadRequest>,
) -> AppResult<Json<NotificationsReadResponse>> {
    if payload.ids.is_empty() {
        return Err(AppError::bad_request("ids must not be empty"));
    }

    let requested: HashSet<Uuid> = payload.ids.into_iter().collect();
    let visible_unread_ids = load_visible_notifications(&state, auth.user_id)
        .await?
        .into_iter()
        .filter(|notification| notification.unread && requested.contains(&notification.id))
        .map(|notification| notification.id)
        .collect::<Vec<_>>();

    let updated = mark_notification_ids_read(&state, &visible_unread_ids).await?;

    let unread_count = count_visible_unread_notifications(&state.pool, auth.user_id).await?;
    push_notification_counter_event(&state, auth.user_id, "read", unread_count, updated).await?;

    Ok(Json(NotificationsReadResponse {
        updated,
        unread_count,
    }))
}

#[utoipa::path(
    post,
    path = "/notifications/read-all",
    tag = "Notifications",
    responses((status = 200, description = "All visible notifications marked as read", body = NotificationsReadResponse)),
    security(("bearerAuth" = []))
)]
pub async fn mark_notifications_read_all(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<NotificationsReadResponse>> {
    let visible_unread_ids = load_visible_notifications(&state, auth.user_id)
        .await?
        .into_iter()
        .filter(|notification| notification.unread)
        .map(|notification| notification.id)
        .collect::<Vec<_>>();
    let updated = mark_notification_ids_read(&state, &visible_unread_ids).await?;

    let unread_count = count_visible_unread_notifications(&state.pool, auth.user_id).await?;
    push_notification_counter_event(&state, auth.user_id, "read_all", unread_count, updated)
        .await?;

    Ok(Json(NotificationsReadResponse {
        updated,
        unread_count,
    }))
}

fn visible_notifications_select_sql() -> String {
    let id_case = uuid_sql::case_uuid("n.id");
    let event_case = uuid_sql::case_uuid("n.event_id");
    let project_case = uuid_sql::case_uuid("n.project_id");
    let actor_case = uuid_sql::case_uuid("n.actor_id");
    let entity_case = uuid_sql::case_uuid("n.entity_id");
    let notification_project = uuid_sql::expr_uuid("n.project_id");
    let project_id = uuid_sql::expr_uuid("p.id");
    let notification_entity = uuid_sql::expr_uuid("n.entity_id");
    let task_id = uuid_sql::expr_uuid("t.id");
    format!(
        "SELECT
            {id_case},
            {event_case},
            {project_case},
            {actor_case},
            n.actor_name AS actor_name,
            n.entity_type AS entity_type,
            {entity_case},
            n.change_type AS change_type,
            p.name AS project_name,
            t.title AS task_title,
            n.occurred_at AS occurred_at,
            n.read_at AS read_at,
            n.created_at AS created_at
         FROM notifications n
         LEFT JOIN projects p
           ON {notification_project} IS NOT NULL
          AND {project_id} = {notification_project}
          AND p.deleted_at IS NULL
         LEFT JOIN tasks t
           ON n.entity_type = 'task'
          AND {notification_entity} IS NOT NULL
          AND {task_id} = {notification_entity}
          AND t.deleted_at IS NULL
         WHERE {recipient_match}
           AND n.deleted_at IS NULL
         ORDER BY (n.read_at IS NULL) DESC, n.occurred_at DESC",
        recipient_match = uuid_sql::match_uuid_clause("n.user_id")
    )
}

fn notification_from_row(row: &sqlx::sqlite::SqliteRow) -> AppResult<Notification> {
    let id: String = row.try_get("id")?;
    let event_id: String = row.try_get("event_id")?;
    let project_id: Option<String> = row.try_get("project_id")?;
    let actor_id: Option<String> = row.try_get("actor_id")?;
    let entity_id: Option<String> = row.try_get("entity_id")?;
    let project_name: Option<String> = row.try_get("project_name")?;
    let task_title: Option<String> = row.try_get("task_title")?;
    let occurred_at: String = row.try_get("occurred_at")?;
    let read_at: Option<String> = row.try_get("read_at")?;
    let created_at: String = row.try_get("created_at")?;
    let actor_name: String = row.try_get("actor_name")?;
    let entity_type: String = row.try_get("entity_type")?;
    let change_type: String = row.try_get("change_type")?;
    let project_id = project_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| AppError::internal(format!("invalid project id: {}", error)))?;
    let entity_id = entity_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| AppError::internal(format!("invalid entity id: {}", error)))?;
    let title = notification_title(&entity_type, &change_type, task_title.as_deref());
    let message = notification_message(
        &actor_name,
        &entity_type,
        &change_type,
        task_title.as_deref(),
        project_name.as_deref(),
    );
    let route = notification_route(project_id, &entity_type, entity_id);
    let severity = notification_severity(&change_type);

    Ok(Notification {
        id: Uuid::parse_str(&id)
            .map_err(|error| AppError::internal(format!("invalid notification id: {}", error)))?,
        event_id: Uuid::parse_str(&event_id)
            .map_err(|error| AppError::internal(format!("invalid event id: {}", error)))?,
        project_id,
        project_name,
        actor: NotificationActor {
            id: actor_id
                .as_deref()
                .map(Uuid::parse_str)
                .transpose()
                .map_err(|error| AppError::internal(format!("invalid actor id: {}", error)))?
                .unwrap_or_else(Uuid::nil),
            name: actor_name,
        },
        entity_type,
        entity_id,
        change_type,
        title,
        message,
        route,
        severity,
        occurred_at: parse_db_datetime(&occurred_at)?,
        read_at: read_at.as_deref().map(parse_db_datetime).transpose()?,
        unread: read_at.is_none(),
        created_at: parse_db_datetime(&created_at)?,
    })
}

async fn load_visible_notifications(
    state: &AppState,
    user_id: Uuid,
) -> AppResult<Vec<Notification>> {
    let sql = visible_notifications_select_sql();
    let rows = sqlx::query(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    let mut project_visibility = HashMap::new();

    for row in rows {
        let notification = notification_from_row(&row)?;
        if notification_is_visible(
            state,
            user_id,
            notification.project_id,
            &mut project_visibility,
        )
        .await?
        {
            items.push(notification);
        }
    }

    Ok(items)
}

async fn notification_is_visible(
    state: &AppState,
    user_id: Uuid,
    project_id: Option<Uuid>,
    project_visibility: &mut HashMap<Uuid, bool>,
) -> AppResult<bool> {
    let Some(project_id) = project_id else {
        return Ok(true);
    };

    if let Some(is_visible) = project_visibility.get(&project_id) {
        return Ok(*is_visible);
    }

    let is_visible = user_can_view_project(&state.pool, user_id, project_id).await?;
    project_visibility.insert(project_id, is_visible);
    Ok(is_visible)
}

async fn mark_notification_ids_read(state: &AppState, ids: &[Uuid]) -> AppResult<usize> {
    if ids.is_empty() {
        return Ok(0);
    }

    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "UPDATE notifications
         SET read_at = COALESCE(read_at, ?)
         WHERE id IN ({placeholders})"
    );

    let mut query = sqlx::query(&sql).bind(Utc::now());
    for id in ids {
        query = query.bind(id.to_string());
    }

    Ok(query.execute(&state.pool).await?.rows_affected() as usize)
}

async fn push_notification_counter_event(
    state: &AppState,
    user_id: Uuid,
    change_type: &str,
    unread_count: i64,
    updated: usize,
) -> AppResult<()> {
    let actor = fetch_actor_info(&state.pool, user_id)
        .await
        .unwrap_or_else(|_| system_actor());
    state
        .realtime_hub
        .send_to_user(
            user_id,
            RealtimeEvent {
                family: RealtimeEventFamily::Notification,
                project_id: None,
                event_id: Uuid::new_v4(),
                actor,
                entity_type: "notification".to_string(),
                entity_id: None,
                change_type: change_type.to_string(),
                occurred_at: Utc::now(),
                unread_count: Some(unread_count),
                metadata: Some(RealtimeEventMetadata::NotificationCounter(
                    RealtimeNotificationCounterMetadata { updated },
                )),
            },
        )
        .await;
    Ok(())
}
