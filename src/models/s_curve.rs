use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SCurveMetric {
    Progress,
    Hours,
    Cost,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SCurveStage {
    Lag,
    Log,
    Maturity,
    Decline,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SCurveDataStatus {
    Ok,
    InsufficientData,
    UnsupportedMetric,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Rule5070Status {
    UnsupportedMetric,
    InsufficientElapsedTimeData,
    InsufficientProgressData,
    InsufficientMetricData,
    PreWindow,
    Pass,
    Fail,
    PostWindowPass,
    PostWindowFail,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SCurveHealthResponse {
    pub metric: SCurveMetric,
    pub metric_supported: bool,
    pub data_status: SCurveDataStatus,
    pub elapsed_time_pct: Option<f64>,
    pub planned_pct: Option<f64>,
    pub actual_pct: Option<f64>,
    pub variance_pct: Option<f64>,
    pub stage: Option<SCurveStage>,
    pub rule_50_70_pass: Option<bool>,
    pub rule_50_70_status: Rule5070Status,
    pub planned_source: Option<String>,
    pub actual_source: Option<String>,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioSCurveProjectSummary {
    pub project_id: Uuid,
    pub project_name: String,
    pub metric_supported: bool,
    pub data_status: SCurveDataStatus,
    pub elapsed_time_pct: Option<f64>,
    pub planned_pct: Option<f64>,
    pub actual_pct: Option<f64>,
    pub variance_pct: Option<f64>,
    pub stage: Option<SCurveStage>,
    pub rule_50_70_pass: Option<bool>,
    pub rule_50_70_status: Rule5070Status,
    pub planned_source: Option<String>,
    pub actual_source: Option<String>,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioSCurveSummaryResponse {
    pub metric: SCurveMetric,
    pub metric_supported: bool,
    pub data_status: SCurveDataStatus,
    pub planned_source: Option<String>,
    pub actual_source: Option<String>,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub project_count: usize,
    pub avg_planned_pct: Option<f64>,
    pub avg_actual_pct: Option<f64>,
    pub avg_variance_pct: Option<f64>,
    pub lag_count: usize,
    pub log_count: usize,
    pub maturity_count: usize,
    pub decline_count: usize,
    pub projects: Vec<PortfolioSCurveProjectSummary>,
}
