use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub status: String,
    pub due_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub start_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub end_date: Option<DateTime<Utc>>,
    pub duration_days: Option<i32>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub progress: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DbTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub status: String,
    pub due_date: Option<DateTime<Utc>>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub duration_days: Option<i32>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub progress: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<DbTask> for Task {
    type Error = AppError;

    fn try_from(value: DbTask) -> Result<Self, Self::Error> {
        Ok(Task {
            id: value.id,
            project_id: value.project_id,
            title: value.title,
            status: value.status,
            due_date: value.due_date,
            start_date: value.start_date,
            end_date: value.end_date,
            duration_days: value.duration_days,
            assignee: value.assignee,
            parent_id: value.parent_id,
            progress: value.progress,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskCreateRequest {
    #[schema(example = "Define launch checklist")]
    pub title: String,
    #[schema(example = "pending")]
    pub status: Option<String>,
    #[schema(format = DateTime, example = "2025-10-10T10:00:00Z")]
    pub due_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-01T09:00:00Z")]
    pub start_date: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-15T17:00:00Z")]
    pub end_date: Option<DateTime<Utc>>,
    pub assignee: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    #[schema(example = 0)]
    pub progress: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskUpdateRequest {
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
