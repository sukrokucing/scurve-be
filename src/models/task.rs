use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskProgressMethod {
    #[default]
    ManualPercentLegacy,
    WeightedComponents,
}

impl TaskProgressMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManualPercentLegacy => "manual_percent_legacy",
            Self::WeightedComponents => "weighted_components",
        }
    }
}

impl FromStr for TaskProgressMethod {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "manual_percent_legacy" => Ok(Self::ManualPercentLegacy),
            "weighted_components" => Ok(Self::WeightedComponents),
            other => Err(format!("unsupported progress_method '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskScheduleStatus {
    FinishedEarly,
    Overdue,
    OnTime,
    NotSpecified,
}

impl FromStr for TaskScheduleStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "finished_early" => Ok(Self::FinishedEarly),
            "overdue" => Ok(Self::Overdue),
            "on_time" => Ok(Self::OnTime),
            "not_specified" => Ok(Self::NotSpecified),
            other => Err(format!("unsupported schedule_status '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionStatus {
    NotStarted,
    InProgress,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskHealthStatus {
    Ahead,
    OnTrack,
    AtRisk,
    Critical,
    NeedsPlan,
}

impl TaskHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ahead => "ahead",
            Self::OnTrack => "on_track",
            Self::AtRisk => "at_risk",
            Self::Critical => "critical",
            Self::NeedsPlan => "needs_plan",
        }
    }

    pub fn rank(self) -> i32 {
        match self {
            Self::Critical => 0,
            Self::AtRisk => 1,
            Self::NeedsPlan => 2,
            Self::OnTrack => 3,
            Self::Ahead => 4,
        }
    }
}

impl FromStr for TaskHealthStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ahead" => Ok(Self::Ahead),
            "on_track" => Ok(Self::OnTrack),
            "at_risk" => Ok(Self::AtRisk),
            "critical" => Ok(Self::Critical),
            "needs_plan" => Ok(Self::NeedsPlan),
            other => Err(format!("unsupported health_status '{}'", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    #[schema(example = "[Quick Add] Define launch checklist")]
    pub description: String,
    #[schema(example = "pending")]
    pub status: String,
    pub progress_method: TaskProgressMethod,
    #[schema(example = false)]
    pub blocked_flag: bool,
    #[schema(example = "Waiting for dependency sign-off")]
    pub blocked_reason: Option<String>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub baseline_start_at: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub baseline_end_at: Option<DateTime<Utc>>,
    #[schema(example = 1.0)]
    pub task_weight: f64,
    pub due_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub start_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub end_date: Option<DateTime<Utc>>,
    pub duration_days: Option<i32>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    #[schema(example = 45)]
    pub progress: i32,
    #[schema(format = DateTime, example = "2025-10-12T14:30:00Z")]
    pub completed_at: Option<DateTime<Utc>>,
    #[schema(example = false)]
    pub completed_at_is_backfilled: bool,
    #[schema(example = "on_time")]
    pub schedule_status: TaskScheduleStatus,
    #[schema(example = "in_progress")]
    pub execution_status: TaskExecutionStatus,
    #[schema(example = 40.0)]
    pub expected_progress_pct: Option<f64>,
    #[schema(example = 45.0)]
    pub actual_progress_pct: Option<f64>,
    #[schema(example = 5.0)]
    pub variance_pct: Option<f64>,
    #[schema(example = "on_track")]
    pub health_status: TaskHealthStatus,
    #[schema(example = "tasks.baseline_linear")]
    pub expected_progress_source: Option<String>,
    #[schema(example = "tasks.progress")]
    pub actual_progress_source: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl crate::events::Loggable for Task {
    fn entity_type() -> &'static str {
        "task"
    }
    fn subject_id(&self) -> Uuid {
        self.id
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub progress_method: TaskProgressMethod,
    pub blocked_flag: bool,
    pub blocked_reason: Option<String>,
    pub baseline_start_at: Option<DateTime<Utc>>,
    pub baseline_end_at: Option<DateTime<Utc>>,
    pub task_weight: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub duration_days: Option<i32>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub progress: i32,
    pub completed_at: Option<DateTime<Utc>>,
    pub completed_at_is_backfilled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub fn is_terminal_task_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "done" | "completed" | "closed"
    )
}

pub fn is_task_completed(status: &str, progress: i32, completed_at: Option<DateTime<Utc>>) -> bool {
    completed_at.is_some() || progress >= 100 || is_terminal_task_status(status)
}

pub fn managed_completed_at(
    status: &str,
    progress: i32,
    current_completed_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if is_task_completed(status, progress, current_completed_at) {
        Some(current_completed_at.unwrap_or(now))
    } else {
        None
    }
}

pub fn managed_completed_at_is_backfilled(
    current_completed_at: Option<DateTime<Utc>>,
    current_completed_at_is_backfilled: bool,
    next_completed_at: Option<DateTime<Utc>>,
) -> bool {
    match (current_completed_at, next_completed_at) {
        (_, None) => false,
        (Some(current), Some(next)) if current == next => current_completed_at_is_backfilled,
        _ => false,
    }
}

pub fn effective_completion_at(
    status: &str,
    progress: i32,
    completed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if let Some(completed_at) = completed_at {
        Some(completed_at)
    } else if is_task_completed(status, progress, completed_at) {
        Some(updated_at)
    } else {
        None
    }
}

pub fn compute_task_schedule_status(
    due_date: Option<DateTime<Utc>>,
    status: &str,
    progress: i32,
    completed_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> TaskScheduleStatus {
    let Some(due_date) = due_date else {
        return TaskScheduleStatus::NotSpecified;
    };

    let due_day = due_date.date_naive();
    if let Some(completed_at) = effective_completion_at(status, progress, completed_at, updated_at)
    {
        let completed_day = completed_at.date_naive();
        if completed_day < due_day {
            TaskScheduleStatus::FinishedEarly
        } else if completed_day > due_day {
            TaskScheduleStatus::Overdue
        } else {
            TaskScheduleStatus::OnTime
        }
    } else if now.date_naive() > due_day {
        TaskScheduleStatus::Overdue
    } else {
        TaskScheduleStatus::OnTime
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TaskCreateRequest {
    #[schema(example = "Define launch checklist")]
    pub title: String,
    #[schema(example = "[Quick Add] Define launch checklist")]
    pub description: Option<String>,
    #[schema(example = "pending")]
    pub status: Option<String>,
    #[schema(example = "manual_percent_legacy")]
    pub progress_method: Option<TaskProgressMethod>,
    #[schema(example = false)]
    pub blocked_flag: Option<bool>,
    #[schema(example = "Waiting for stakeholder review")]
    pub blocked_reason: Option<String>,
    #[schema(format = DateTime, example = "2025-10-10T10:00:00Z")]
    pub due_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub start_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub end_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub baseline_start_at: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub baseline_end_at: Option<DateTime<Utc>>,
    #[schema(example = 1.0)]
    pub task_weight: Option<f64>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    #[schema(example = 0)]
    pub progress: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskUpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub progress_method: Option<TaskProgressMethod>,
    pub blocked_flag: Option<bool>,
    pub blocked_reason: Option<String>,
    #[schema(format = DateTime, example = "2025-11-01T10:00:00Z")]
    pub due_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub start_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub end_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub baseline_start_at: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub baseline_end_at: Option<DateTime<Utc>>,
    pub task_weight: Option<f64>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub progress: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskBatchUpdateRequest {
    pub id: Uuid,
    pub title: Option<String>,
    pub status: Option<String>,
    #[schema(format = DateTime, example = "2025-11-01T10:00:00Z")]
    pub due_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub start_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub end_date: Option<DateTime<Utc>>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub progress: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskBatchUpdatePayload {
    pub tasks: Vec<TaskBatchUpdateRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskBatchDeleteRequest {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskBatchDeleteResponse {
    pub deleted: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskAssignee {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskActivityEntry {
    pub id: String,
    pub action: String,
    pub actor_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    #[schema(value_type = Object)]
    pub details: Value,
}
