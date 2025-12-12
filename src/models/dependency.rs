use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskDependency {
    pub id: Uuid,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub type_: String, // "finish_to_start"
    pub created_at: DateTime<Utc>,
}

impl crate::events::Loggable for TaskDependency {
    fn entity_type() -> &'static str { "dependency" }
    fn subject_id(&self) -> Uuid { self.id }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbTaskDependency {
    pub id: Uuid,
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    pub type_: String,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<DbTaskDependency> for TaskDependency {
    type Error = AppError;

    fn try_from(value: DbTaskDependency) -> Result<Self, Self::Error> {
        Ok(TaskDependency {
            id: value.id,
            source_task_id: value.source_task_id,
            target_task_id: value.target_task_id,
            type_: value.type_,
            created_at: value.created_at,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DependencyCreateRequest {
    pub source_task_id: Uuid,
    pub target_task_id: Uuid,
    #[serde(default = "default_type")]
    pub type_: String,
}

fn default_type() -> String {
    "finish_to_start".to_string()
}
