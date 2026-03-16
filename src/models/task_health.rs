use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::task::TaskHealthStatus;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskHealthRule {
    pub health_status: TaskHealthStatus,
    #[schema(example = -25.0)]
    pub variance_from: Option<f64>,
    #[schema(example = -10.0)]
    pub variance_to: Option<f64>,
    #[schema(example = 1)]
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct EffectiveTaskHealthRule {
    pub health_status: TaskHealthStatus,
    pub variance_from: Option<f64>,
    pub variance_to: Option<f64>,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskHealthRuleSetResponse {
    #[schema(example = "project")]
    pub scope: String,
    pub project_id: Option<Uuid>,
    pub rules: Vec<TaskHealthRule>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TaskHealthRuleInput {
    pub health_status: TaskHealthStatus,
    #[schema(example = -25.0)]
    pub variance_from: Option<f64>,
    #[schema(example = -10.0)]
    pub variance_to: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateTaskHealthRulesRequest {
    pub rules: Vec<TaskHealthRuleInput>,
}

#[derive(Debug, Clone)]
pub struct EffectiveTaskHealthRuleSet {
    pub scope: String,
    pub project_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
    pub rules: Vec<EffectiveTaskHealthRule>,
}
