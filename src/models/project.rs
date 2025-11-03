use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Project {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub theme_color: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DbProject {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub theme_color: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<DbProject> for Project {
    type Error = AppError;

    fn try_from(value: DbProject) -> Result<Self, Self::Error> {
        Ok(Project {
            id: value.id,
            user_id: value.user_id,
            name: value.name,
            description: value.description,
            theme_color: value.theme_color,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectCreateRequest {
    #[schema(example = "Launch Planning")]
    pub name: String,
    #[schema(example = "Prepare milestones for the product launch.")]
    pub description: Option<String>,
    #[schema(example = "#3498db")]
    pub theme_color: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectUpdateRequest {
    #[schema(example = "Launch Planning")]
    pub name: Option<String>,
    #[schema(example = "Updated description")]
    pub description: Option<String>,
    #[schema(example = "#2ecc71")]
    pub theme_color: Option<String>,
}
