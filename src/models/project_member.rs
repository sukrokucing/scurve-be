use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::resource_role::ResourceRoleRef;

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectMember {
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: String,
    pub access_role_id: Uuid,
    pub access_role_name: String,
    pub resource_roles: Vec<ResourceRoleRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectMemberCreateRequest {
    pub user_id: Uuid,
    pub access_role_id: Uuid,
    pub resource_role_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MyProjectScopeSummary {
    pub project_id: Uuid,
    pub project_name: String,
    pub access_role_id: Uuid,
    pub access_role_name: String,
    pub resource_roles: Vec<ResourceRoleRef>,
    pub permissions: Vec<String>,
}
