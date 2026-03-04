use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventName {
    #[serde(rename = "time_to_task.session_started")]
    SessionStarted,
    #[serde(rename = "time_to_task.intent_marked")]
    IntentMarked,
    #[serde(rename = "time_to_task.completed")]
    Completed,
    #[serde(rename = "time_to_task.abandoned")]
    Abandoned,
}

impl TelemetryEventName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStarted => "time_to_task.session_started",
            Self::IntentMarked => "time_to_task.intent_marked",
            Self::Completed => "time_to_task.completed",
            Self::Abandoned => "time_to_task.abandoned",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TelemetryEventRequest {
    #[schema(example = "time-to-task-start-c6f53fe6-1823-4f90-b03f-77e8e20de34d")]
    pub event_id: String,
    pub event_name: TelemetryEventName,
    #[schema(format = DateTime, example = "2026-03-04T05:38:12.232Z")]
    pub occurred_at: DateTime<Utc>,
    #[schema(example = "c6f53fe6-1823-4f90-b03f-77e8e20de34d")]
    pub session_id: Option<String>,
    #[schema(example = "/tasks")]
    pub route: Option<String>,
    pub user_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    #[schema(example = "list")]
    pub view: Option<String>,
    #[schema(example = "completed")]
    pub outcome: Option<String>,
    #[schema(example = "leave-tasks-page")]
    pub reason: Option<String>,
    #[schema(example = 9234, minimum = 0)]
    pub duration_ms: Option<i64>,
    #[schema(example = 4033, minimum = 0)]
    pub intent_to_complete_ms: Option<i64>,
    #[schema(value_type = Object, example = json!({"source":"tasks-page"}))]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TelemetryBatchRequest {
    #[schema(min_items = 1, max_items = 20)]
    pub events: Vec<TelemetryEventRequest>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TelemetryIngestResponse {
    #[schema(example = 12)]
    pub accepted: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TelemetryErrorResponse {
    #[schema(example = "invalid telemetry payload")]
    pub message: String,
}
