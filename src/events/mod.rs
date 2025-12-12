use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;
use sqlx::SqlitePool;

pub mod loggable;
pub use loggable::{Loggable, Severity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent<T> {
    pub id: Uuid,
    pub name: &'static str,
    pub occurred_at: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub payload: T,
}

impl<T> DomainEvent<T> {
    pub fn new(name: &'static str, actor_id: Option<Uuid>, subject_id: Option<Uuid>, payload: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            occurred_at: Utc::now(),
            actor_id,
            subject_id,
            payload,
        }
    }
}

pub type EventBus = broadcast::Sender<Value>;

pub fn init_event_bus() -> (EventBus, broadcast::Receiver<Value>) {
    broadcast::channel(1024)
}

/// Request context for activity logging (IP, User-Agent, etc.)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[allow(dead_code)]
impl RequestContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Extract context from Axum request headers
    pub fn from_headers(headers: &axum::http::HeaderMap) -> Self {
        let ip = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .or_else(|| {
                headers
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(String::from)
            });

        let user_agent = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        Self { ip, user_agent }
    }

    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip = Some(ip.into());
        self
    }

    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }
}

/// Structured activity payload following Phase 4 convention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityPayload {
    /// The current/new state of the entity
    #[serde(rename = "new")]
    pub current: Value,
    /// The previous state (for update/delete operations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Value>,
    /// Request context (IP, User-Agent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<RequestContext>,
    /// Severity level for retention policy
    pub severity: Severity,
}

/// Helper function to log activity for any entity implementing `Loggable`.
/// This reduces boilerplate in handlers.
///
/// # Arguments
/// * `event_bus` - The event bus to send the event to.
/// * `action` - The action performed (e.g., "created", "updated", "deleted").
/// * `actor_id` - The user who performed the action.
/// * `entity` - The entity being logged (must implement `Loggable`).
#[allow(dead_code)]
pub fn log_activity<T: Loggable>(
    event_bus: &EventBus,
    action: &str,
    actor_id: Option<Uuid>,
    entity: &T,
) {
    log_activity_with_context(event_bus, action, actor_id, entity, None, None);
}

/// Enhanced activity logging with old/new tracking and request context.
///
/// # Arguments
/// * `event_bus` - The event bus to send the event to.
/// * `action` - The action performed (e.g., "created", "updated", "deleted").
/// * `actor_id` - The user who performed the action.
/// * `entity` - The current/new entity state (must implement `Loggable`).
/// * `old_entity` - Optional previous entity state (for updates/deletes).
/// * `context` - Optional request context (IP, User-Agent).
pub fn log_activity_with_context<T: Loggable>(
    event_bus: &EventBus,
    action: &str,
    actor_id: Option<Uuid>,
    entity: &T,
    old_entity: Option<&T>,
    context: Option<RequestContext>,
) {
    // Build event name like "task.created"
    let event_name = format!("{}.{}", T::entity_type(), action);

    // We need a 'static lifetime for name, so we leak the string.
    // This is acceptable because event names are a small, bounded set.
    let static_name: &'static str = Box::leak(event_name.into_boxed_str());

    // Build structured payload with dynamic severity
    let severity = entity.severity_for_action(action);
    let payload = ActivityPayload {
        current: serde_json::to_value(entity).unwrap_or_default(),
        old: old_entity.map(|e| serde_json::to_value(e).unwrap_or_default()),
        context,
        severity,
    };

    let event = DomainEvent::new(
        static_name,
        actor_id,
        Some(entity.subject_id()),
        serde_json::to_value(&payload).unwrap_or_default(),
    );

    // Fire and forget - logging failures should not break the API
    let _ = event_bus.send(serde_json::to_value(event).unwrap_or_default());
}

pub async fn start_activity_listener(mut rx: broadcast::Receiver<Value>, pool: SqlitePool) {
    tracing::info!("Activity listener started");
    while let Ok(event) = rx.recv().await {
        // We clone the event to use it for properties JSON, while extracting fields for columns
        let event_json = event.clone();

        // Basic extraction (tolerant)
        let name = event.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let actor_id_str = event.get("actor_id").and_then(|v| v.as_str());
        let subject_id_str = event.get("subject_id").and_then(|v| v.as_str());
        let occurred_at_str = event.get("occurred_at").and_then(|v| v.as_str());

        let description = match name {
            "task.created" => "Task created",
            "task.updated" => "Task updated",
            "task.deleted" => "Task deleted",
            "project.created" => "Project created",
            "project.updated" => "Project updated",
            "project.deleted" => "Project deleted",
            "user.registered" => "New user registered",
            "user.login" => "User logged in",
            _ => "System event",
        }.to_string();

        // Extract severity from payload (Phase 5)
        let severity = event
            .get("payload")
            .and_then(|p| p.get("severity"))
            .and_then(|s| s.as_str())
            .unwrap_or("important");

        // We store actor_id and subject_id as proper UUIDs if they parse, otherwise NULL
        let actor_id = actor_id_str.and_then(|s| Uuid::parse_str(s).ok());
        let subject_id = subject_id_str.and_then(|s| Uuid::parse_str(s).ok());

        // Ensure we have a valid timestamp, or default to now
        let occurred_at = occurred_at_str
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let id = Uuid::new_v4();

        // Phase 3: Insert into activity_log (projection)
        let result = sqlx::query!(
            r#"
            INSERT INTO activity_log (id, event_name, description, actor_id, subject_id, occurred_at, properties, severity)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            id,
            name,
            description,
            actor_id,
            subject_id,
            occurred_at,
            event_json,
            severity
        )
        .execute(&pool)
        .await;

        if let Err(e) = result {
            tracing::error!("Failed to save activity log: {}", e);
        }

        // Phase 6: Insert into event_store with hash chain
        let event_store_id = Uuid::new_v4();
        let payload_str = serde_json::to_string(&event_json).unwrap_or_default();

        // Get the previous hash from the last event
        let prev_hash_result: Option<String> = sqlx::query_scalar(
            "SELECT hash FROM event_store ORDER BY created_at DESC LIMIT 1"
        )
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        // Compute SHA256(prev_hash || payload)
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        if let Some(ref ph) = prev_hash_result {
            hasher.update(ph.as_bytes());
        }
        hasher.update(payload_str.as_bytes());
        let hash = hex::encode(hasher.finalize());

        let actor_id_str_for_store = actor_id.map(|u| u.to_string());
        let subject_id_str_for_store = subject_id.map(|u| u.to_string());
        let event_store_id_str = event_store_id.to_string();

        let event_store_result = sqlx::query(
            r#"
            INSERT INTO event_store (id, event_name, occurred_at, actor_id, subject_id, payload, severity, prev_hash, hash)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&event_store_id_str)
        .bind(name)
        .bind(occurred_at)
        .bind(&actor_id_str_for_store)
        .bind(&subject_id_str_for_store)
        .bind(&payload_str)
        .bind(severity)
        .bind(&prev_hash_result)
        .bind(&hash)
        .execute(&pool)
        .await;

        if let Err(e) = event_store_result {
            tracing::error!("Failed to save to event store: {}", e);
        }
    }
}
