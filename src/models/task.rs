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
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskUpdateRequest {
    pub title: Option<String>,
    pub status: Option<String>,
    #[schema(format = DateTime, example = "2025-11-01T10:00:00Z")]
    pub due_date: Option<DateTime<Utc>>,
}
