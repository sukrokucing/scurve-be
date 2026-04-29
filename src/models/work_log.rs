use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkLogSource {
    Manual,
    MigratedTaskProgress,
    /// Automatically created when a task's progress moves forward.
    /// Calculated from duration_days × WORKING_HOURS_PER_DAY × progress_delta / 100.
    AutoProgress,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkLog {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub resource_role_id: Uuid,
    pub resource_role_name: String,
    pub hours: f64,
    pub hourly_rate_snapshot: f64,
    pub currency_snapshot: String,
    pub cost_amount: f64,
    #[schema(example = "2026-03-10")]
    pub work_date: String,
    pub note: Option<String>,
    pub source: WorkLogSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl crate::events::Loggable for WorkLog {
    fn entity_type() -> &'static str {
        "work_log"
    }

    fn subject_id(&self) -> Uuid {
        self.id
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkLogCreateRequest {
    pub user_id: Option<Uuid>,
    pub resource_role_id: Uuid,
    #[schema(example = 2.5)]
    pub hours: f64,
    #[schema(example = "2026-03-10")]
    pub work_date: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkLogUpdateRequest {
    pub resource_role_id: Option<Uuid>,
    pub hours: Option<f64>,
    #[schema(example = "2026-03-10")]
    pub work_date: Option<String>,
    pub note: Option<String>,
}
