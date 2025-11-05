use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Progress {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub progress: i32,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DbProgress {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub progress: i32,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<DbProgress> for Progress {
    type Error = AppError;

    fn try_from(value: DbProgress) -> Result<Self, Self::Error> {
        Ok(Progress {
            id: value.id,
            project_id: value.project_id,
            task_id: value.task_id,
            progress: value.progress,
            note: value.note,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProgressCreateRequest {
    #[schema(example = 75)]
    pub progress: i32,
    #[schema(example = "Halfway done - waiting on review")]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProgressUpdateRequest {
    pub progress: Option<i32>,
    pub note: Option<String>,
}
