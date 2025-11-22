use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectPlanPoint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub date: DateTime<Utc>,
    pub planned_progress: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DbProjectPlanPoint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub date: DateTime<Utc>,
    pub planned_progress: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<DbProjectPlanPoint> for ProjectPlanPoint {
    type Error = AppError;

    fn try_from(value: DbProjectPlanPoint) -> Result<Self, Self::Error> {
        Ok(ProjectPlanPoint {
            id: value.id,
            project_id: value.project_id,
            date: value.date,
            planned_progress: value.planned_progress,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectPlanCreateRequest {
    #[schema(example = "2025-12-01T00:00:00Z")]
    pub date: DateTime<Utc>,
    #[schema(example = 10)]
    pub planned_progress: i32,
}
