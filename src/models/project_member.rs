use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectMember {
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: String,
    pub role_id: Uuid,
    pub role_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectMemberCreateRequest {
    pub user_id: Uuid,
    pub role_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MyProjectScopeSummary {
    pub project_id: Uuid,
    pub project_name: String,
    pub role_id: Uuid,
    pub role_name: String,
    pub permissions: Vec<String>,
}
