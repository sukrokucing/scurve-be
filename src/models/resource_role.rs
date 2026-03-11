use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct ResourceRoleRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResourceRole {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub default_hourly_rate: f64,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResourceRoleCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub default_hourly_rate: f64,
    pub currency: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResourceRoleUpdateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_hourly_rate: Option<f64>,
    pub currency: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectResourceRoleRate {
    pub resource_role_id: Uuid,
    pub resource_role_name: String,
    pub hourly_rate: f64,
    pub currency: String,
    pub is_override: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectResourceRoleRateUpsertRequest {
    pub hourly_rate: f64,
    pub currency: String,
}
