use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskProgressComponent {
    pub id: Uuid,
    pub task_id: Uuid,
    #[schema(example = "Requirements signed off")]
    pub name: String,
    #[schema(example = "milestone")]
    pub component_type: String,
    #[schema(example = 40.0)]
    pub weight: f64,
    #[schema(example = 100.0)]
    pub completion_pct: f64,
    #[schema(format = DateTime, example = "2025-10-03T09:00:00Z")]
    pub planned_at: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-04T14:00:00Z")]
    pub completed_at: Option<DateTime<Utc>>,
    #[schema(example = 1)]
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DbTaskProgressComponent {
    pub id: Uuid,
    pub task_id: Uuid,
    pub name: String,
    pub component_type: String,
    pub weight: f64,
    pub completion_pct: f64,
    pub planned_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl From<DbTaskProgressComponent> for TaskProgressComponent {
    fn from(value: DbTaskProgressComponent) -> Self {
        Self {
            id: value.id,
            task_id: value.task_id,
            name: value.name,
            component_type: value.component_type,
            weight: value.weight,
            completion_pct: value.completion_pct,
            planned_at: value.planned_at,
            completed_at: value.completed_at,
            sort_order: value.sort_order,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TaskProgressComponentInput {
    pub id: Option<Uuid>,
    #[schema(example = "Requirements signed off")]
    pub name: String,
    #[schema(example = "milestone")]
    pub component_type: String,
    #[schema(example = 40.0)]
    pub weight: f64,
    #[schema(example = 100.0)]
    pub completion_pct: f64,
    #[schema(format = DateTime, example = "2025-10-03T09:00:00Z")]
    pub planned_at: Option<DateTime<Utc>>,
    #[schema(format = DateTime, example = "2025-10-04T14:00:00Z")]
    pub completed_at: Option<DateTime<Utc>>,
    #[schema(example = 1)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReplaceTaskProgressComponentsRequest {
    pub components: Vec<TaskProgressComponentInput>,
}
