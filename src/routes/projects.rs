use crate::db::{row_parsers, uuid_sql};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::Row;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::project::{DbProject, Project, ProjectCreateRequest, ProjectUpdateRequest};
use crate::models::project_member::{
    MyProjectScopeSummary, ProjectMember, ProjectMemberCreateRequest,
};
use crate::models::project_plan::ProjectPlanPoint;
use crate::models::s_curve::{
    PortfolioSCurveProjectSummary, PortfolioSCurveSummaryResponse, Rule5070Status,
    SCurveDataStatus, SCurveHealthResponse, SCurveMetric, SCurveStage,
};
use crate::utils::utc_now;
use serde::Serialize;
use utoipa::ToSchema;

const DEFAULT_THEME: &str = "#3498db";
const PROJECT_OWNER_ROLE: &str = "project_owner";
const UNCLASSIFIED_RESOURCE_ROLE_ID: &str = "40000000-0000-0000-0000-000000000001";

#[utoipa::path(
    get,
    path = "/projects",
    tag = "Projects",
    responses((status = 200, description = "List projects", body = [Project])),
    security(("bearerAuth" = []))
)]
pub async fn list_projects(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Project>>> {
    let id_case = uuid_sql::case_uuid("p.id");
    let user_case = uuid_sql::case_uuid("p.user_id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let member_match = uuid_sql::match_uuid_clause("pm.user_id");
    let sql = format!(
        "SELECT {} , {} , p.name, p.description, p.theme_color, p.created_at, p.updated_at, p.deleted_at
         FROM projects p
         WHERE p.deleted_at IS NULL
           AND (
               {}
               OR EXISTS (
                   SELECT 1
                   FROM project_members pm
                   WHERE pm.project_id = p.id
                     AND {}
                     AND pm.deleted_at IS NULL
               )
           )
         ORDER BY p.created_at DESC",
        id_case, user_case, match_owner, member_match
    );

    let rows = sqlx::query(&sql)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut projects = Vec::with_capacity(rows.len());
    for row in rows {
        projects.push(row_parsers::db_project_from_row(&row)?);
    }

    let projects: Vec<Project> = projects
        .into_iter()
        .map(Project::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(projects))
}

#[utoipa::path(
    post,
    path = "/projects",
    tag = "Projects",
    request_body = ProjectCreateRequest,
    responses((status = 201, description = "Project created", body = Project)),
    security(("bearerAuth" = []))
)]
pub async fn create_project(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ProjectCreateRequest>,
) -> AppResult<(StatusCode, Json<Project>)> {
    let now = utc_now();
    let project_id = Uuid::new_v4();
    let theme_color = payload
        .theme_color
        .clone()
        .unwrap_or_else(|| DEFAULT_THEME.to_string());

    let match_user_id = uuid_sql::match_uuid_clause("id");
    let insert_sql = format!(
        "INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) \
         VALUES (?, (SELECT id FROM users WHERE {}), ?, ?, ?, ?, ?)",
        match_user_id
    );

    sqlx::query(&insert_sql)
        .bind(project_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(&payload.name)
        .bind(&payload.description)
        .bind(&theme_color)
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await?;

    let owner_role_id: String = sqlx::query_scalar("SELECT id FROM roles WHERE name = ? LIMIT 1")
        .bind(PROJECT_OWNER_ROLE)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| {
            AppError::internal("project_owner role not found; run latest migrations".to_string())
        })?;

    let match_project = uuid_sql::match_uuid_clause("id");
    let match_member_user = uuid_sql::match_uuid_clause("id");
    let match_actor_user = uuid_sql::match_uuid_clause("id");
    let membership_sql = format!(
        "INSERT INTO project_members (id, project_id, user_id, access_role_id, created_at, created_by, updated_at, updated_by)
         VALUES (
            ?,
            (SELECT id FROM projects WHERE {}),
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
            ?,
            ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
            ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         )",
        match_project, match_member_user, match_actor_user, match_actor_user
    );

    let membership_id = Uuid::new_v4();
    sqlx::query(&membership_sql)
        .bind(membership_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(owner_role_id)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .execute(&state.pool)
        .await?;

    let actor_match = uuid_sql::match_uuid_clause("id");
    let insert_member_role_sql = format!(
        "INSERT INTO project_member_resource_roles (
            id, membership_id, resource_role_id, created_at, created_by, updated_at, updated_by
         ) VALUES (
            ?,
            ?,
            ?,
            ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
            ?,
            (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         )",
        actor_match, actor_match
    );

    sqlx::query(&insert_member_role_sql)
        .bind(Uuid::new_v4().to_string())
        .bind(membership_id.to_string())
        .bind(UNCLASSIFIED_RESOURCE_ROLE_ID)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .execute(&state.pool)
        .await?;

    let project = fetch_project(&state.pool, auth.user_id, project_id).await?;
    let project: Project = project.try_into()?;

    // Log activity with request context
    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "created",
        Some(auth.user_id),
        &project,
        None,
        Some(ctx),
    );

    Ok((StatusCode::CREATED, Json(project)))
}

#[utoipa::path(
    get,
    path = "/projects/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Project detail", body = Project)),
    security(("bearerAuth" = []))
)]
pub async fn get_project(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Project>> {
    let project = fetch_project(&state.pool, auth.user_id, id).await?;
    let project: Project = project.try_into()?;
    Ok(Json(project))
}

#[utoipa::path(
    put,
    path = "/projects/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    request_body = ProjectUpdateRequest,
    responses((status = 200, description = "Project updated", body = Project)),
    security(("bearerAuth" = []))
)]
pub async fn update_project(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProjectUpdateRequest>,
) -> AppResult<Json<Project>> {
    // Capture old state before modifications
    let old_project = fetch_project(&state.pool, auth.user_id, id).await?;
    let old_dto: Project = old_project.clone().try_into()?;

    let mut project = old_project;

    if let Some(name) = payload.name.as_ref() {
        project.name = name.clone();
    }
    if payload.description.is_some() {
        project.description = payload.description.clone();
    }
    if let Some(theme_color) = payload.theme_color.as_ref() {
        project.theme_color = theme_color.clone();
    }

    let now = utc_now();

    let match_id = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "UPDATE projects SET name = ?, description = ?, theme_color = ?, updated_at = ? WHERE {}",
        match_id
    );

    sqlx::query(&sql)
        .bind(&project.name)
        .bind(&project.description)
        .bind(&project.theme_color)
        .bind(now)
        .bind(project.id.to_string())
        .bind(project.id.to_string())
        .execute(&state.pool)
        .await?;

    project.updated_at = now;
    let project: Project = project.try_into()?;

    // Log activity with old/new tracking and request context
    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "updated",
        Some(auth.user_id),
        &project,
        Some(&old_dto),
        Some(ctx),
    );

    Ok(Json(project))
}

#[utoipa::path(
    delete,
    path = "/projects/{id}",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 204, description = "Project soft deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_project(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // Ensure project exists and belongs to user
    let db_project = fetch_project(&state.pool, auth.user_id, id).await?;
    let project: Project = db_project.clone().try_into()?;

    let now = utc_now();
    let match_id = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "UPDATE projects SET deleted_at = ?, updated_at = ? WHERE {} AND deleted_at IS NULL",
        match_id
    );

    let affected = sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .bind(id.to_string())
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("project not found"));
    }

    // Log activity with request context (old state only, no new state for delete)
    let ctx = crate::events::RequestContext::from_headers(&headers);
    crate::events::log_activity_with_context(
        &state.event_bus,
        "deleted",
        Some(auth.user_id),
        &project,
        None,
        Some(ctx),
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_project(pool: &SqlitePool, user_id: Uuid, project_id: Uuid) -> AppResult<DbProject> {
    // Always use the fallback path that handles TEXT UUID storage correctly
    let id_case = uuid_sql::case_uuid("p.id");
    let user_case = uuid_sql::case_uuid("p.user_id");
    let match_id = uuid_sql::match_uuid_clause("p.id");
    let match_owner = uuid_sql::match_uuid_clause("p.user_id");
    let member_match = uuid_sql::match_uuid_clause("pm.user_id");

    let sql = format!(
        "SELECT {} , {} , p.name, p.description, p.theme_color, p.created_at, p.updated_at, p.deleted_at
         FROM projects p
         WHERE {} AND p.deleted_at IS NULL
           AND (
               {}
               OR EXISTS (
                   SELECT 1
                   FROM project_members pm
                   WHERE pm.project_id = p.id
                     AND {}
                     AND pm.deleted_at IS NULL
               )
           )",
        id_case, user_case, match_id, match_owner, member_match
    );

    let row = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;

    if let Some(r) = row {
        row_parsers::db_project_from_row(&r)
    } else {
        Err(AppError::not_found("project not found"))
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActualPoint {
    pub date: String,
    pub actual: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardMetricPoint {
    pub date: String,
    pub value: f64,
}

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    pub metric: Option<SCurveMetric>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardResponse {
    pub project: Project,
    pub plan: Vec<ProjectPlanPoint>,
    pub actual: Vec<ActualPoint>,
    pub metric: SCurveMetric,
    pub metric_supported: bool,
    pub data_status: SCurveDataStatus,
    pub planned_source: Option<String>,
    pub actual_source: Option<String>,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub metric_plan: Vec<DashboardMetricPoint>,
    pub metric_actual: Vec<DashboardMetricPoint>,
}

#[utoipa::path(
    get,
    path = "/projects/{id}/dashboard",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project id"),
        ("metric" = Option<SCurveMetric>, Query, description = "Metric type. Defaults to progress.")
    ),
    responses((status = 200, description = "Project dashboard", body = DashboardResponse)),
    security(("bearerAuth" = []))
)]
pub async fn get_project_dashboard(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<DashboardQuery>,
) -> AppResult<Json<DashboardResponse>> {
    // ensure project exists and belongs to user
    let db_project = fetch_project(&state.pool, auth.user_id, id).await?;
    let project: Project = db_project.try_into()?;
    let metric = query.metric.unwrap_or(SCurveMetric::Progress);

    // Keep legacy dashboard fields for backward compatibility when metric=progress.
    let (plan, actual) = if metric == SCurveMetric::Progress {
        (
            fetch_progress_dashboard_plan(&state.pool, id).await?,
            fetch_progress_dashboard_actual(&state.pool, id).await?,
        )
    } else {
        (Vec::new(), Vec::new())
    };

    let metric_plan = fetch_dashboard_metric_plan_series(&state.pool, id, metric).await?;
    let metric_actual = fetch_dashboard_metric_actual_series(&state.pool, id, metric).await?;
    let data_status = if metric_plan.is_empty() || metric_actual.is_empty() {
        SCurveDataStatus::InsufficientData
    } else {
        SCurveDataStatus::Ok
    };
    let (planned_source, actual_source, unit) = metric_series_metadata(metric);
    let currency = match metric {
        SCurveMetric::Cost => Some(resolve_cost_currency(&state.pool, id).await?),
        _ => None,
    };

    let resp = DashboardResponse {
        project,
        plan,
        actual,
        metric,
        metric_supported: true,
        data_status,
        planned_source: Some(planned_source.to_string()),
        actual_source: Some(actual_source.to_string()),
        unit: Some(unit.to_string()),
        currency,
        metric_plan,
        metric_actual,
    };

    Ok(Json(resp))
}

async fn fetch_progress_dashboard_plan(
    pool: &SqlitePool,
    project_id: Uuid,
) -> AppResult<Vec<ProjectPlanPoint>> {
    let plan_id_case = uuid_sql::case_uuid("id");
    let plan_proj_case = uuid_sql::case_uuid("project_id");
    let plan_proj_match = uuid_sql::match_uuid_clause("project_id");
    let plan_sql = format!(
        "SELECT {} , {} , date, planned_progress, planned_hours, planned_cost, currency, created_at, updated_at FROM project_plan WHERE {} ORDER BY date ASC",
        plan_id_case, plan_proj_case, plan_proj_match
    );

    let plan_rows = sqlx::query(&plan_sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(pool)
        .await?;

    let mut plan_pts = Vec::with_capacity(plan_rows.len());
    for row in plan_rows {
        plan_pts.push(row_parsers::db_project_plan_point_from_row(&row)?);
    }

    plan_pts
        .into_iter()
        .map(ProjectPlanPoint::try_from)
        .collect::<Result<_, _>>()
}

async fn fetch_progress_dashboard_actual(
    pool: &SqlitePool,
    project_id: Uuid,
) -> AppResult<Vec<ActualPoint>> {
    let actual_proj_match = uuid_sql::match_uuid_clause("project_id");
    let actual_sql = format!(
        "SELECT DATE(created_at) as date, CAST(ROUND(AVG(progress)) AS INTEGER) as actual FROM task_progress WHERE {} AND deleted_at IS NULL GROUP BY DATE(created_at) ORDER BY DATE(created_at) ASC",
        actual_proj_match
    );
    let actual_rows = sqlx::query_as::<_, (String, i64)>(&actual_sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(pool)
        .await?;

    Ok(actual_rows
        .into_iter()
        .map(|(date, actual)| ActualPoint {
            date,
            actual: actual as i32,
        })
        .collect())
}

async fn fetch_dashboard_metric_plan_series(
    pool: &SqlitePool,
    project_id: Uuid,
    metric: SCurveMetric,
) -> AppResult<Vec<DashboardMetricPoint>> {
    let column = match metric {
        SCurveMetric::Progress => "planned_progress",
        SCurveMetric::Hours => "planned_hours",
        SCurveMetric::Cost => "planned_cost",
    };

    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT DATE(date) AS date, CAST({column} AS REAL) AS value
         FROM project_plan
         WHERE {match_proj} AND {column} IS NOT NULL
         ORDER BY datetime(date) ASC"
    );

    let rows = sqlx::query_as::<_, (String, f64)>(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(date, value)| DashboardMetricPoint {
            date,
            value: round2(value),
        })
        .collect())
}

async fn fetch_dashboard_metric_actual_series(
    pool: &SqlitePool,
    project_id: Uuid,
    metric: SCurveMetric,
) -> AppResult<Vec<DashboardMetricPoint>> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    match metric {
        SCurveMetric::Progress => {
            let sql = format!(
                "SELECT DATE(created_at) as date, AVG(CAST(progress AS REAL)) as value
                 FROM task_progress
                 WHERE {match_proj} AND deleted_at IS NULL
                 GROUP BY DATE(created_at)
                 ORDER BY DATE(created_at) ASC"
            );
            let rows = sqlx::query_as::<_, (String, f64)>(&sql)
                .bind(project_id.to_string())
                .bind(project_id.to_string())
                .fetch_all(pool)
                .await?;
            Ok(rows
                .into_iter()
                .map(|(date, value)| DashboardMetricPoint {
                    date,
                    value: round2(value),
                })
                .collect())
        }
        SCurveMetric::Hours | SCurveMetric::Cost => {
            let column = if metric == SCurveMetric::Hours {
                "hours"
            } else {
                "cost_amount"
            };
            let sql = format!(
                "SELECT DATE(work_date) as date, SUM(CAST({column} AS REAL)) as value
                 FROM work_logs
                 WHERE {match_proj} AND deleted_at IS NULL
                 GROUP BY DATE(work_date)
                 ORDER BY DATE(work_date) ASC"
            );
            let rows = sqlx::query_as::<_, (String, f64)>(&sql)
                .bind(project_id.to_string())
                .bind(project_id.to_string())
                .fetch_all(pool)
                .await?;

            let mut cumulative = 0.0_f64;
            Ok(rows
                .into_iter()
                .map(|(date, value)| {
                    cumulative += value;
                    DashboardMetricPoint {
                        date,
                        value: round2(cumulative),
                    }
                })
                .collect())
        }
    }
}

fn metric_series_metadata(metric: SCurveMetric) -> (&'static str, &'static str, &'static str) {
    match metric {
        SCurveMetric::Progress => (
            "project_plan.planned_progress",
            "task_progress.progress (avg)",
            "percent",
        ),
        SCurveMetric::Hours => (
            "project_plan.planned_hours (cumulative)",
            "work_logs.hours (sum, cumulative by day)",
            "hours",
        ),
        SCurveMetric::Cost => (
            "project_plan.planned_cost (cumulative)",
            "work_logs.cost_amount (sum, cumulative by day)",
            "currency",
        ),
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CriticalPathResponse {
    pub task_ids: Vec<Uuid>,
}

#[utoipa::path(
    get,
    path = "/projects/{id}/critical-path",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Critical path task ids", body = CriticalPathResponse)),
    security(("bearerAuth" = []))
)]
pub async fn get_project_critical_path(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<CriticalPathResponse>> {
    // ensure project exists and belongs to user
    let _ = fetch_project(&state.pool, auth.user_id, id).await?;

    // Fetch tasks with computed duration (fallback to 0)
    let id_case = uuid_sql::case_uuid("t.id");
    let match_proj = uuid_sql::match_uuid_clause("t.project_id");
    let sql_tasks = format!(
        "SELECT {} , COALESCE(t.duration_days, CAST(julianday(t.end_date) - julianday(t.start_date) AS INTEGER), 0) as duration_days FROM tasks t WHERE {} AND t.deleted_at IS NULL",
        id_case, match_proj
    );

    let task_rows = sqlx::query(&sql_tasks)
        .bind(id.to_string())
        .bind(id.to_string())
        .fetch_all(&state.pool)
        .await?;

    use std::collections::{HashMap, HashSet, VecDeque};

    let mut durations: HashMap<Uuid, i32> = HashMap::new();
    let mut nodes: HashSet<Uuid> = HashSet::new();
    for row in task_rows.iter() {
        let id_s: String = row
            .try_get("id")
            .map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
        let dur: i64 = row
            .try_get("duration_days")
            .map_err(|e| AppError::internal(format!("missing duration_days: {}", e)))?;
        let tu = Uuid::parse_str(&id_s)
            .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
        durations.insert(tu, dur as i32);
        nodes.insert(tu);
    }

    // Fetch dependencies (edges source -> target)
    let id_case_s = uuid_sql::case_uuid("d.source_task_id");
    let id_case_t = uuid_sql::case_uuid("d.target_task_id");
    let project_match = uuid_sql::match_uuid_clause("t.project_id");
    let sql_deps = format!(
        "SELECT {} , {} FROM task_dependencies d INNER JOIN tasks t ON t.id = d.source_task_id WHERE {} AND t.deleted_at IS NULL",
        id_case_s, id_case_t, project_match
    );

    let dep_rows = sqlx::query(&sql_deps)
        .bind(id.to_string())
        .bind(id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut indeg: HashMap<Uuid, usize> = HashMap::new();
    for n in nodes.iter() {
        indeg.insert(*n, 0);
    }

    for row in dep_rows.iter() {
        let src_s: String = row
            .try_get("source_task_id")
            .map_err(|e| AppError::internal(format!("missing source_task_id: {}", e)))?;
        let tgt_s: String = row
            .try_get("target_task_id")
            .map_err(|e| AppError::internal(format!("missing target_task_id: {}", e)))?;
        let src = Uuid::parse_str(&src_s)
            .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
        let tgt = Uuid::parse_str(&tgt_s)
            .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
        if !nodes.contains(&src) || !nodes.contains(&tgt) {
            continue;
        }
        adj.entry(src).or_default().push(tgt);
        *indeg.entry(tgt).or_default() += 1;
    }

    // Kahn's algorithm for topological order
    let mut q: VecDeque<Uuid> = VecDeque::new();
    for (&n, &d) in indeg.iter() {
        if d == 0 {
            q.push_back(n);
        }
    }

    let mut topo: Vec<Uuid> = Vec::new();
    while let Some(n) = q.pop_front() {
        topo.push(n);
        if let Some(neis) = adj.get(&n) {
            for &m in neis {
                if let Some(e) = indeg.get_mut(&m) {
                    *e -= 1;
                    if *e == 0 {
                        q.push_back(m);
                    }
                }
            }
        }
    }

    if topo.len() != nodes.len() {
        return Err(AppError::internal(
            "dependency graph is not a DAG".to_string(),
        ));
    }

    // DP for longest path (by duration). Initialize best[node] = duration[node]
    let mut best: HashMap<Uuid, i64> = HashMap::new();
    let mut prev: HashMap<Uuid, Option<Uuid>> = HashMap::new();
    for &n in topo.iter() {
        best.insert(n, durations.get(&n).cloned().unwrap_or(0) as i64);
        prev.insert(n, None);
    }

    for &u in topo.iter() {
        let bu = *best.get(&u).unwrap_or(&0);
        if let Some(neis) = adj.get(&u) {
            for &v in neis {
                let cand = bu + durations.get(&v).cloned().unwrap_or(0) as i64;
                if cand > *best.get(&v).unwrap_or(&0) {
                    best.insert(v, cand);
                    prev.insert(v, Some(u));
                }
            }
        }
    }

    // Find node with max best value
    let mut max_node: Option<Uuid> = None;
    let mut max_val: i64 = -1;
    for (&n, &val) in best.iter() {
        if val > max_val {
            max_val = val;
            max_node = Some(n);
        }
    }

    let mut path: Vec<Uuid> = Vec::new();
    if let Some(mut cur) = max_node {
        while let Some(p) = prev.get(&cur).and_then(|o| *o) {
            path.push(cur);
            cur = p;
        }
        path.push(cur);
        path.reverse();
    }

    Ok(Json(CriticalPathResponse { task_ids: path }))
}

#[utoipa::path(
    post,
    path = "/projects/{id}/plan",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    request_body = [ProjectPlanCreateRequest],
    responses((status = 200, description = "Project plan updated", body = [ProjectPlanPoint])),
    security(("bearerAuth" = []))
)]
pub async fn update_project_plan(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<Vec<crate::models::project_plan::ProjectPlanCreateRequest>>,
) -> AppResult<Json<Vec<ProjectPlanPoint>>> {
    // ensure project exists and belongs to user
    let _ = fetch_project(&state.pool, auth.user_id, id).await?;

    let mut tx = state.pool.begin().await?;
    let now = utc_now();

    // 1. Clear existing plan
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let delete_plan_sql = format!("DELETE FROM project_plan WHERE {}", match_proj);
    sqlx::query(&delete_plan_sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;

    // 2. Insert new points
    let mut inserted_ids = Vec::new();
    for point in payload {
        if point.planned_progress < 0 || point.planned_progress > 100 {
            return Err(AppError::bad_request(
                "planned_progress must be between 0 and 100",
            ));
        }
        if let Some(planned_hours) = point.planned_hours {
            if planned_hours < 0.0 {
                return Err(AppError::bad_request("planned_hours must be non-negative"));
            }
        }
        if let Some(planned_cost) = point.planned_cost {
            if planned_cost < 0.0 {
                return Err(AppError::bad_request("planned_cost must be non-negative"));
            }
        }
        let currency = normalize_currency(point.currency)?;

        let pid = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO project_plan (id, project_id, date, planned_progress, planned_hours, planned_cost, currency, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(pid.to_string())
        .bind(id.to_string())
        .bind(point.date)
        .bind(point.planned_progress)
        .bind(point.planned_hours)
        .bind(point.planned_cost)
        .bind(currency)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        inserted_ids.push(pid);
    }

    tx.commit().await?;

    // 3. Fetch and return new plan
    let plan_id_case = uuid_sql::case_uuid("id");
    let plan_proj_case = uuid_sql::case_uuid("project_id");
    let plan_proj_match = uuid_sql::match_uuid_clause("project_id");
    let plan_sql = format!(
        "SELECT {} , {} , date, planned_progress, planned_hours, planned_cost, currency, created_at, updated_at FROM project_plan WHERE {} ORDER BY date ASC",
        plan_id_case, plan_proj_case, plan_proj_match
    );

    let plan_rows = sqlx::query(&plan_sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut plan_pts = Vec::with_capacity(plan_rows.len());
    for row in plan_rows {
        plan_pts.push(row_parsers::db_project_plan_point_from_row(&row)?);
    }

    let plan: Vec<ProjectPlanPoint> = plan_pts
        .into_iter()
        .map(ProjectPlanPoint::try_from)
        .collect::<Result<_, _>>()?;

    Ok(Json(plan))
}

#[utoipa::path(
    delete,
    path = "/projects/{id}/plan",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 204, description = "Project plan cleared")),
    security(("bearerAuth" = []))
)]
pub async fn clear_project_plan(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // ensure project exists and belongs to user
    let _ = fetch_project(&state.pool, auth.user_id, id).await?;

    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let delete_plan_sql = format!("DELETE FROM project_plan WHERE {}", match_proj);
    sqlx::query(&delete_plan_sql)
        .bind(id.to_string())
        .bind(id.to_string())
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/projects/{project_id}/members",
    tag = "Projects",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List active project members", body = [ProjectMember])),
    security(("bearerAuth" = []))
)]
pub async fn list_project_members(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<ProjectMember>>> {
    let _ = fetch_project(&state.pool, auth.user_id, project_id).await?;

    let membership_case = uuid_sql::case_uuid("pm.id");
    let user_case = uuid_sql::case_uuid("pm.user_id");
    let role_case = uuid_sql::case_uuid("pm.access_role_id");
    let match_proj = uuid_sql::match_uuid_clause("pm.project_id");
    let sql = format!(
        "SELECT
            {} ,
            {} ,
            u.name AS user_name,
            u.email AS user_email,
            {} ,
            r.name AS access_role_name,
            pm.created_at,
            pm.updated_at
         FROM project_members pm
         INNER JOIN users u ON u.id = pm.user_id
         INNER JOIN roles r ON r.id = pm.access_role_id
         WHERE {} AND pm.deleted_at IS NULL AND u.deleted_at IS NULL
         ORDER BY u.name ASC",
        membership_case, user_case, role_case, match_proj
    );

    let rows = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let membership_id: String = row.try_get("id")?;
        let user_id: String = row.try_get("user_id")?;
        let access_role_id: String = row.try_get("access_role_id")?;
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;

        let membership_id = Uuid::parse_str(&membership_id)
            .map_err(|e| AppError::internal(format!("invalid membership uuid: {}", e)))?;
        let resource_roles = fetch_member_resource_roles(&state.pool, membership_id).await?;

        items.push(ProjectMember {
            user_id: Uuid::parse_str(&user_id)
                .map_err(|e| AppError::internal(format!("invalid user_id uuid: {}", e)))?,
            user_name: row.try_get("user_name")?,
            user_email: row.try_get("user_email")?,
            access_role_id: Uuid::parse_str(&access_role_id)
                .map_err(|e| AppError::internal(format!("invalid access role uuid: {}", e)))?,
            access_role_name: row.try_get("access_role_name")?,
            resource_roles,
            created_at: parse_db_datetime(&created_at)?,
            updated_at: parse_db_datetime(&updated_at)?,
        });
    }

    Ok(Json(items))
}

#[utoipa::path(
    post,
    path = "/projects/{project_id}/members",
    tag = "Projects",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = ProjectMemberCreateRequest,
    responses((status = 201, description = "Project member added or updated", body = ProjectMember)),
    security(("bearerAuth" = []))
)]
pub async fn create_project_member(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<ProjectMemberCreateRequest>,
) -> AppResult<(StatusCode, Json<ProjectMember>)> {
    let _ = fetch_project(&state.pool, auth.user_id, project_id).await?;
    ensure_user_exists(&state.pool, payload.user_id).await?;
    ensure_role_exists(&state.pool, payload.access_role_id).await?;
    if payload.resource_role_ids.is_empty() {
        return Err(AppError::bad_request(
            "resource_role_ids must contain at least one role",
        ));
    }
    ensure_resource_roles_exist(&state.pool, &payload.resource_role_ids).await?;

    let now = utc_now();
    let match_project = uuid_sql::match_uuid_clause("project_id");
    let match_user = uuid_sql::match_uuid_clause("user_id");
    let existing_sql = format!(
        "SELECT id, deleted_at
         FROM project_members
         WHERE {} AND {}
         ORDER BY created_at DESC
         LIMIT 1",
        match_project, match_user
    );
    let existing = sqlx::query(&existing_sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(payload.user_id.to_string())
        .bind(payload.user_id.to_string())
        .fetch_optional(&state.pool)
        .await?;

    let membership_id = if let Some(row) = existing {
        let id: String = row.try_get("id")?;
        let deleted_at: Option<String> = row.try_get("deleted_at")?;
        let membership_id = Uuid::parse_str(&id)
            .map_err(|e| AppError::internal(format!("invalid membership id: {}", e)))?;

        if deleted_at.is_some() {
            let match_id = uuid_sql::match_uuid_clause("id");
            let actor_match = uuid_sql::match_uuid_clause("id");
            let sql = format!(
                "UPDATE project_members
                 SET access_role_id = ?, deleted_at = NULL, deleted_by = NULL, updated_at = ?,
                     updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
                 WHERE {}",
                actor_match, match_id
            );
            sqlx::query(&sql)
                .bind(payload.access_role_id.to_string())
                .bind(now)
                .bind(auth.user_id.to_string())
                .bind(auth.user_id.to_string())
                .bind(id.clone())
                .bind(id)
                .execute(&state.pool)
                .await?;
        } else {
            let match_id = uuid_sql::match_uuid_clause("id");
            let actor_match = uuid_sql::match_uuid_clause("id");
            let sql = format!(
                "UPDATE project_members
                 SET access_role_id = ?, updated_at = ?,
                     updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
                 WHERE {}",
                actor_match, match_id
            );
            sqlx::query(&sql)
                .bind(payload.access_role_id.to_string())
                .bind(now)
                .bind(auth.user_id.to_string())
                .bind(auth.user_id.to_string())
                .bind(id.clone())
                .bind(id)
                .execute(&state.pool)
                .await?;
        }

        membership_id
    } else {
        let membership_id = Uuid::new_v4();
        let project_match = uuid_sql::match_uuid_clause("id");
        let user_match = uuid_sql::match_uuid_clause("id");
        let actor_match = uuid_sql::match_uuid_clause("id");
        let insert_sql = format!(
            "INSERT INTO project_members (
                id, project_id, user_id, access_role_id, created_at, created_by, updated_at, updated_by
             ) VALUES (
                ?,
                (SELECT id FROM projects WHERE {} AND deleted_at IS NULL),
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
                ?,
                ?,
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
                ?,
                (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
             )",
            project_match, user_match, actor_match, actor_match
        );

        sqlx::query(&insert_sql)
            .bind(membership_id.to_string())
            .bind(project_id.to_string())
            .bind(project_id.to_string())
            .bind(payload.user_id.to_string())
            .bind(payload.user_id.to_string())
            .bind(payload.access_role_id.to_string())
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .execute(&state.pool)
            .await?;

        membership_id
    };

    replace_member_resource_roles(
        &state.pool,
        membership_id,
        &payload.resource_role_ids,
        auth.user_id,
    )
    .await?;

    let member = fetch_project_member(&state.pool, project_id, payload.user_id).await?;
    Ok((StatusCode::CREATED, Json(member)))
}

#[utoipa::path(
    delete,
    path = "/projects/{project_id}/members/{user_id}",
    tag = "Projects",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("user_id" = Uuid, Path, description = "User id")
    ),
    responses((status = 204, description = "Project member soft deleted")),
    security(("bearerAuth" = []))
)]
pub async fn delete_project_member(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((project_id, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let _ = fetch_project(&state.pool, auth.user_id, project_id).await?;

    let now = utc_now();
    let match_project = uuid_sql::match_uuid_clause("project_id");
    let match_user = uuid_sql::match_uuid_clause("user_id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "UPDATE project_members
         SET deleted_at = ?,
             deleted_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
             updated_at = ?,
             updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         WHERE {} AND {} AND deleted_at IS NULL",
        actor_match, actor_match, match_project, match_user
    );
    let affected = sqlx::query(&sql)
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(now)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .execute(&state.pool)
        .await?;

    if affected.rows_affected() == 0 {
        return Err(AppError::not_found("project member not found"));
    }

    let membership_case = uuid_sql::case_uuid("id");
    let lookup_sql = format!(
        "SELECT {}
         FROM project_members
         WHERE {} AND {}
         ORDER BY updated_at DESC
         LIMIT 1",
        membership_case, match_project, match_user
    );

    if let Some(membership_id) = sqlx::query_scalar::<_, String>(&lookup_sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&state.pool)
        .await?
    {
        let match_membership = uuid_sql::match_uuid_clause("membership_id");
        let actor_match = uuid_sql::match_uuid_clause("id");
        let sql = format!(
            "UPDATE project_member_resource_roles
             SET deleted_at = ?,
                 deleted_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
                 updated_at = ?,
                 updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
             WHERE {} AND deleted_at IS NULL",
            actor_match, actor_match, match_membership
        );
        sqlx::query(&sql)
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .bind(now)
            .bind(auth.user_id.to_string())
            .bind(auth.user_id.to_string())
            .bind(membership_id.clone())
            .bind(membership_id)
            .execute(&state.pool)
            .await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/users/me/projects",
    tag = "Projects",
    responses((status = 200, description = "Accessible projects with effective scoped permissions", body = [MyProjectScopeSummary])),
    security(("bearerAuth" = []))
)]
pub async fn list_my_project_scopes(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<MyProjectScopeSummary>>> {
    let user_match = uuid_sql::match_uuid_clause("pm.user_id");
    let membership_case = uuid_sql::case_uuid("pm.id");
    let project_case = uuid_sql::case_uuid("pm.project_id");
    let role_case = uuid_sql::case_uuid("pm.access_role_id");
    let sql = format!(
        "SELECT
            {} ,
            {} ,
            p.name AS project_name,
            {} ,
            r.name AS access_role_name,
            perm.name AS permission_name
         FROM project_members pm
         INNER JOIN projects p ON p.id = pm.project_id
         INNER JOIN roles r ON r.id = pm.access_role_id
         LEFT JOIN role_permissions rp ON rp.role_id = pm.access_role_id
         LEFT JOIN permissions perm ON perm.id = rp.permission_id
         WHERE {} AND pm.deleted_at IS NULL AND p.deleted_at IS NULL
         ORDER BY p.name ASC, perm.name ASC",
        membership_case, project_case, role_case, user_match
    );

    let rows = sqlx::query(&sql)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut grouped: std::collections::BTreeMap<String, (Uuid, MyProjectScopeSummary)> =
        std::collections::BTreeMap::new();

    for row in rows {
        let membership_id_s: String = row.try_get("id")?;
        let project_id_s: String = row.try_get("project_id")?;
        let access_role_id_s: String = row.try_get("access_role_id")?;
        let membership_id = Uuid::parse_str(&membership_id_s)
            .map_err(|e| AppError::internal(format!("invalid membership id: {}", e)))?;
        let project_id = Uuid::parse_str(&project_id_s)
            .map_err(|e| AppError::internal(format!("invalid project id: {}", e)))?;
        let access_role_id = Uuid::parse_str(&access_role_id_s)
            .map_err(|e| AppError::internal(format!("invalid access role id: {}", e)))?;
        let permission_name: Option<String> = row.try_get("permission_name")?;

        let entry = grouped.entry(project_id_s.clone()).or_insert_with(|| {
            (
                membership_id,
                MyProjectScopeSummary {
                    project_id,
                    project_name: row.try_get("project_name").unwrap_or_default(),
                    access_role_id,
                    access_role_name: row.try_get("access_role_name").unwrap_or_default(),
                    resource_roles: Vec::new(),
                    permissions: Vec::new(),
                },
            )
        });
        if let Some(permission_name) = permission_name {
            if !entry.1.permissions.contains(&permission_name) {
                entry.1.permissions.push(permission_name);
            }
        }
    }

    let mut items = Vec::with_capacity(grouped.len());
    for (_, (membership_id, mut scope)) in grouped {
        scope.resource_roles = fetch_member_resource_roles(&state.pool, membership_id).await?;
        scope.permissions.sort();
        items.push(scope);
    }
    items.sort_by(|a, b| a.project_name.cmp(&b.project_name));
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub struct SCurveHealthQuery {
    pub metric: Option<SCurveMetric>,
}

#[utoipa::path(
    get,
    path = "/projects/{id}/s-curve/health",
    tag = "Projects",
    params(
        ("id" = Uuid, Path, description = "Project id"),
        ("metric" = Option<SCurveMetric>, Query, description = "Metric type. Defaults to progress.")
    ),
    responses((status = 200, description = "Project S-curve health", body = SCurveHealthResponse)),
    security(("bearerAuth" = []))
)]
pub async fn get_project_s_curve_health(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<SCurveHealthQuery>,
) -> AppResult<Json<SCurveHealthResponse>> {
    let _ = fetch_project(&state.pool, auth.user_id, id).await?;
    let metric = query.metric.unwrap_or(SCurveMetric::Progress);
    let response = compute_project_s_curve_health(&state.pool, id, metric).await?;
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub struct PortfolioSummaryQuery {
    pub metric: Option<SCurveMetric>,
}

#[utoipa::path(
    get,
    path = "/portfolio/s-curve/summary",
    tag = "Projects",
    params(("metric" = Option<SCurveMetric>, Query, description = "Metric type. Defaults to progress.")),
    responses((status = 200, description = "Portfolio S-curve summary", body = PortfolioSCurveSummaryResponse)),
    security(("bearerAuth" = []))
)]
pub async fn get_portfolio_s_curve_summary(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<PortfolioSummaryQuery>,
) -> AppResult<Json<PortfolioSCurveSummaryResponse>> {
    let metric = query.metric.unwrap_or(SCurveMetric::Progress);
    let user_match = uuid_sql::match_uuid_clause("pm.user_id");
    let project_case = uuid_sql::case_uuid("p.id");
    let sql = format!(
        "SELECT {} , p.name
         FROM project_members pm
         INNER JOIN projects p ON p.id = pm.project_id
         WHERE {} AND pm.deleted_at IS NULL AND p.deleted_at IS NULL
         ORDER BY p.name ASC",
        project_case, user_match
    );
    let rows = sqlx::query(&sql)
        .bind(auth.user_id.to_string())
        .bind(auth.user_id.to_string())
        .fetch_all(&state.pool)
        .await?;

    let mut projects = Vec::with_capacity(rows.len());
    for row in rows {
        let project_id_s: String = row.try_get("id")?;
        let project_id = Uuid::parse_str(&project_id_s)
            .map_err(|e| AppError::internal(format!("invalid project id: {}", e)))?;
        let project_name: String = row.try_get("name")?;
        let health = compute_project_s_curve_health(&state.pool, project_id, metric).await?;
        projects.push(PortfolioSCurveProjectSummary {
            project_id,
            project_name,
            metric_supported: health.metric_supported,
            data_status: health.data_status,
            elapsed_time_pct: health.elapsed_time_pct,
            planned_pct: health.planned_pct,
            actual_pct: health.actual_pct,
            variance_pct: health.variance_pct,
            stage: health.stage,
            rule_50_70_pass: health.rule_50_70_pass,
            rule_50_70_status: health.rule_50_70_status,
            planned_source: health.planned_source.clone(),
            actual_source: health.actual_source.clone(),
            unit: health.unit.clone(),
            currency: health.currency.clone(),
            last_updated_at: health.last_updated_at,
        });
    }

    let avg_planned_pct = avg_option(projects.iter().map(|p| p.planned_pct));
    let avg_actual_pct = avg_option(projects.iter().map(|p| p.actual_pct));
    let avg_variance_pct = avg_option(projects.iter().map(|p| p.variance_pct));

    let mut lag_count = 0usize;
    let mut log_count = 0usize;
    let mut maturity_count = 0usize;
    let mut decline_count = 0usize;
    for project in &projects {
        match project.stage {
            Some(SCurveStage::Lag) => lag_count += 1,
            Some(SCurveStage::Log) => log_count += 1,
            Some(SCurveStage::Maturity) => maturity_count += 1,
            Some(SCurveStage::Decline) => decline_count += 1,
            _ => {}
        }
    }

    let summary_data_status = if projects
        .iter()
        .any(|project| project.data_status == SCurveDataStatus::Ok)
    {
        SCurveDataStatus::Ok
    } else {
        SCurveDataStatus::InsufficientData
    };

    Ok(Json(PortfolioSCurveSummaryResponse {
        metric,
        metric_supported: true,
        data_status: summary_data_status,
        planned_source: projects
            .iter()
            .find_map(|project| project.planned_source.clone()),
        actual_source: projects
            .iter()
            .find_map(|project| project.actual_source.clone()),
        unit: projects.iter().find_map(|project| project.unit.clone()),
        currency: projects.iter().find_map(|project| project.currency.clone()),
        project_count: projects.len(),
        avg_planned_pct,
        avg_actual_pct,
        avg_variance_pct,
        lag_count,
        log_count,
        maturity_count,
        decline_count,
        projects,
    }))
}

async fn fetch_project_member(
    pool: &SqlitePool,
    project_id: Uuid,
    user_id: Uuid,
) -> AppResult<ProjectMember> {
    let membership_case = uuid_sql::case_uuid("pm.id");
    let user_case = uuid_sql::case_uuid("pm.user_id");
    let role_case = uuid_sql::case_uuid("pm.access_role_id");
    let match_project = uuid_sql::match_uuid_clause("pm.project_id");
    let match_user = uuid_sql::match_uuid_clause("pm.user_id");
    let sql = format!(
        "SELECT
            {} ,
            {} ,
            u.name AS user_name,
            u.email AS user_email,
            {} ,
            r.name AS access_role_name,
            pm.created_at,
            pm.updated_at
         FROM project_members pm
         INNER JOIN users u ON u.id = pm.user_id
         INNER JOIN roles r ON r.id = pm.access_role_id
         WHERE {} AND {} AND pm.deleted_at IS NULL
         LIMIT 1",
        membership_case, user_case, role_case, match_project, match_user
    );
    let row = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("project member not found"))?;

    let membership_id: String = row.try_get("id")?;
    let user_id: String = row.try_get("user_id")?;
    let access_role_id: String = row.try_get("access_role_id")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;
    let membership_id = Uuid::parse_str(&membership_id)
        .map_err(|e| AppError::internal(format!("invalid membership uuid: {}", e)))?;
    let resource_roles = fetch_member_resource_roles(pool, membership_id).await?;

    Ok(ProjectMember {
        user_id: Uuid::parse_str(&user_id)
            .map_err(|e| AppError::internal(format!("invalid user_id uuid: {}", e)))?,
        user_name: row.try_get("user_name")?,
        user_email: row.try_get("user_email")?,
        access_role_id: Uuid::parse_str(&access_role_id)
            .map_err(|e| AppError::internal(format!("invalid access role uuid: {}", e)))?,
        access_role_name: row.try_get("access_role_name")?,
        resource_roles,
        created_at: parse_db_datetime(&created_at)?,
        updated_at: parse_db_datetime(&updated_at)?,
    })
}

async fn fetch_member_resource_roles(
    pool: &SqlitePool,
    membership_id: Uuid,
) -> AppResult<Vec<crate::models::resource_role::ResourceRoleRef>> {
    let role_case = uuid_sql::case_uuid("rr.id");
    let match_membership = uuid_sql::match_uuid_clause("pmrr.membership_id");
    let sql = format!(
        "SELECT {} , rr.name
         FROM project_member_resource_roles pmrr
         INNER JOIN resource_roles rr ON rr.id = pmrr.resource_role_id
         WHERE {} AND pmrr.deleted_at IS NULL AND rr.deleted_at IS NULL
         ORDER BY rr.name ASC",
        role_case, match_membership
    );
    let rows = sqlx::query(&sql)
        .bind(membership_id.to_string())
        .bind(membership_id.to_string())
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        out.push(crate::models::resource_role::ResourceRoleRef {
            id: Uuid::parse_str(&id)
                .map_err(|e| AppError::internal(format!("invalid resource role uuid: {}", e)))?,
            name: row.try_get("name")?,
        });
    }
    Ok(out)
}

async fn ensure_user_exists(pool: &SqlitePool, user_id: Uuid) -> AppResult<()> {
    let match_user = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "SELECT 1 FROM users WHERE {} AND deleted_at IS NULL",
        match_user
    );
    let exists: Option<i64> = sqlx::query_scalar(&sql)
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("user not found"));
    }
    Ok(())
}

async fn ensure_resource_roles_exist(pool: &SqlitePool, role_ids: &[Uuid]) -> AppResult<()> {
    for role_id in role_ids {
        let match_role = uuid_sql::match_uuid_clause("id");
        let sql = format!(
            "SELECT 1 FROM resource_roles WHERE {} AND deleted_at IS NULL",
            match_role
        );
        let exists: Option<i64> = sqlx::query_scalar(&sql)
            .bind(role_id.to_string())
            .bind(role_id.to_string())
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::not_found(format!(
                "resource role {} not found",
                role_id
            )));
        }
    }
    Ok(())
}

async fn replace_member_resource_roles(
    pool: &SqlitePool,
    membership_id: Uuid,
    role_ids: &[Uuid],
    actor_id: Uuid,
) -> AppResult<()> {
    let now = utc_now();
    let membership_match = uuid_sql::match_uuid_clause("membership_id");
    let actor_match = uuid_sql::match_uuid_clause("id");
    let deactivate_sql = format!(
        "UPDATE project_member_resource_roles
         SET deleted_at = ?,
             deleted_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
             updated_at = ?,
             updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
         WHERE {} AND deleted_at IS NULL",
        actor_match, actor_match, membership_match
    );
    sqlx::query(&deactivate_sql)
        .bind(now)
        .bind(actor_id.to_string())
        .bind(actor_id.to_string())
        .bind(now)
        .bind(actor_id.to_string())
        .bind(actor_id.to_string())
        .bind(membership_id.to_string())
        .bind(membership_id.to_string())
        .execute(pool)
        .await?;

    for role_id in role_ids {
        let match_membership = uuid_sql::match_uuid_clause("membership_id");
        let match_role = uuid_sql::match_uuid_clause("resource_role_id");
        let existing_sql = format!(
            "SELECT id
             FROM project_member_resource_roles
             WHERE {} AND {}
             ORDER BY created_at DESC
             LIMIT 1",
            match_membership, match_role
        );
        let existing: Option<String> = sqlx::query_scalar(&existing_sql)
            .bind(membership_id.to_string())
            .bind(membership_id.to_string())
            .bind(role_id.to_string())
            .bind(role_id.to_string())
            .fetch_optional(pool)
            .await?;

        if let Some(id) = existing {
            let match_id = uuid_sql::match_uuid_clause("id");
            let actor_match = uuid_sql::match_uuid_clause("id");
            let sql = format!(
                "UPDATE project_member_resource_roles
                 SET deleted_at = NULL,
                     deleted_by = NULL,
                     updated_at = ?,
                     updated_by = (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
                 WHERE {}",
                actor_match, match_id
            );
            sqlx::query(&sql)
                .bind(now)
                .bind(actor_id.to_string())
                .bind(actor_id.to_string())
                .bind(id.clone())
                .bind(id)
                .execute(pool)
                .await?;
        } else {
            let actor_match = uuid_sql::match_uuid_clause("id");
            let insert_sql = format!(
                "INSERT INTO project_member_resource_roles (
                    id, membership_id, resource_role_id, created_at, created_by, updated_at, updated_by
                 ) VALUES (
                    ?,
                    ?,
                    ?,
                    ?,
                    (SELECT id FROM users WHERE {} AND deleted_at IS NULL),
                    ?,
                    (SELECT id FROM users WHERE {} AND deleted_at IS NULL)
                 )",
                actor_match, actor_match
            );
            sqlx::query(&insert_sql)
                .bind(Uuid::new_v4().to_string())
                .bind(membership_id.to_string())
                .bind(role_id.to_string())
                .bind(now)
                .bind(actor_id.to_string())
                .bind(actor_id.to_string())
                .bind(now)
                .bind(actor_id.to_string())
                .bind(actor_id.to_string())
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

async fn ensure_role_exists(pool: &SqlitePool, role_id: Uuid) -> AppResult<()> {
    let match_role = uuid_sql::match_uuid_clause("id");
    let sql = format!("SELECT 1 FROM roles WHERE {}", match_role);
    let exists: Option<i64> = sqlx::query_scalar(&sql)
        .bind(role_id.to_string())
        .bind(role_id.to_string())
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("role not found"));
    }
    Ok(())
}

async fn compute_project_s_curve_health(
    pool: &SqlitePool,
    project_id: Uuid,
    metric: SCurveMetric,
) -> AppResult<SCurveHealthResponse> {
    let elapsed_time_pct = compute_elapsed_time_pct(pool, project_id).await?;
    let last_updated_at = compute_last_updated_at(pool, project_id).await?;
    let metric_values = compute_metric_values(pool, project_id, metric).await?;
    let planned_pct = metric_values.planned_pct;
    let actual_pct = metric_values.actual_pct;
    let variance_pct = match (actual_pct, planned_pct) {
        (Some(actual), Some(planned)) => Some(round2(actual - planned)),
        _ => None,
    };
    let (rule_50_70_pass, rule_50_70_status) =
        compute_rule_50_70_status(metric, elapsed_time_pct, planned_pct, actual_pct);
    let stage = resolve_stage(
        pool,
        project_id,
        metric,
        elapsed_time_pct,
        planned_pct,
        actual_pct,
        variance_pct,
        rule_50_70_pass,
    )
    .await?;

    let data_status = if planned_pct.is_some() && actual_pct.is_some() {
        SCurveDataStatus::Ok
    } else {
        SCurveDataStatus::InsufficientData
    };

    Ok(SCurveHealthResponse {
        metric,
        metric_supported: true,
        data_status,
        elapsed_time_pct: elapsed_time_pct.map(round2),
        planned_pct: planned_pct.map(round2),
        actual_pct: actual_pct.map(round2),
        variance_pct: variance_pct.map(round2),
        stage,
        rule_50_70_pass,
        rule_50_70_status,
        planned_source: Some(metric_values.planned_source),
        actual_source: Some(metric_values.actual_source),
        unit: Some(metric_values.unit),
        currency: metric_values.currency,
        last_updated_at,
    })
}

async fn compute_elapsed_time_pct(pool: &SqlitePool, project_id: Uuid) -> AppResult<Option<f64>> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT
            CASE
                WHEN MIN(date) IS NULL OR MAX(date) IS NULL THEN NULL
                WHEN julianday(MAX(date)) <= julianday(MIN(date)) THEN NULL
                ELSE MIN(
                    100.0,
                    MAX(
                        0.0,
                        ((julianday('now') - julianday(MIN(date))) / (julianday(MAX(date)) - julianday(MIN(date)))) * 100.0
                    )
                )
            END AS elapsed_time_pct
         FROM project_plan
         WHERE {}",
        match_proj
    );

    let value: Option<f64> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(pool)
        .await?;
    Ok(value)
}

async fn compute_planned_progress_pct(
    pool: &SqlitePool,
    project_id: Uuid,
) -> AppResult<Option<f64>> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT CAST(planned_progress AS REAL)
         FROM project_plan
         WHERE {}
         ORDER BY
            CASE WHEN datetime(date) <= datetime('now') THEN 0 ELSE 1 END,
            ABS(julianday(date) - julianday('now')) ASC
         LIMIT 1",
        match_proj
    );

    let value: Option<f64> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(pool)
        .await?
        .flatten();
    Ok(value)
}

async fn compute_actual_progress_pct(
    pool: &SqlitePool,
    project_id: Uuid,
) -> AppResult<(Option<f64>, &'static str)> {
    let match_proj_tasks = uuid_sql::match_uuid_clause("project_id");
    let sql_tasks = format!(
        "SELECT AVG(CAST(progress AS REAL)) FROM tasks WHERE {} AND deleted_at IS NULL",
        match_proj_tasks
    );
    let tasks_avg: Option<f64> = sqlx::query_scalar(&sql_tasks)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(pool)
        .await?;

    if tasks_avg.is_some() {
        return Ok((tasks_avg, "tasks.progress (avg)"));
    }

    let match_proj_progress = uuid_sql::match_uuid_clause("project_id");
    let sql_progress = format!(
        "SELECT AVG(CAST(progress AS REAL)) FROM task_progress WHERE {} AND deleted_at IS NULL",
        match_proj_progress
    );
    let progress_avg: Option<f64> = sqlx::query_scalar(&sql_progress)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(pool)
        .await?;
    Ok((progress_avg, "task_progress.progress (avg)"))
}

struct MetricValues {
    planned_pct: Option<f64>,
    actual_pct: Option<f64>,
    planned_source: String,
    actual_source: String,
    unit: String,
    currency: Option<String>,
}

async fn compute_metric_values(
    pool: &SqlitePool,
    project_id: Uuid,
    metric: SCurveMetric,
) -> AppResult<MetricValues> {
    match metric {
        SCurveMetric::Progress => {
            let planned_pct = compute_planned_progress_pct(pool, project_id).await?;
            let (actual_pct, actual_source) = compute_actual_progress_pct(pool, project_id).await?;
            Ok(MetricValues {
                planned_pct,
                actual_pct,
                planned_source: "project_plan.planned_progress".to_string(),
                actual_source: actual_source.to_string(),
                unit: "percent".to_string(),
                currency: None,
            })
        }
        SCurveMetric::Hours => {
            let (planned_pct, actual_pct) =
                compute_relative_metric_pct(pool, project_id, "planned_hours", "hours").await?;
            Ok(MetricValues {
                planned_pct,
                actual_pct,
                planned_source: "project_plan.planned_hours (cumulative)".to_string(),
                actual_source: "work_logs.hours (sum)".to_string(),
                unit: "hours".to_string(),
                currency: None,
            })
        }
        SCurveMetric::Cost => {
            let (planned_pct, actual_pct) =
                compute_relative_metric_pct(pool, project_id, "planned_cost", "cost_amount")
                    .await?;
            Ok(MetricValues {
                planned_pct,
                actual_pct,
                planned_source: "project_plan.planned_cost (cumulative)".to_string(),
                actual_source: "work_logs.cost_amount (sum)".to_string(),
                unit: "currency".to_string(),
                currency: Some(resolve_cost_currency(pool, project_id).await?),
            })
        }
    }
}

async fn compute_relative_metric_pct(
    pool: &SqlitePool,
    project_id: Uuid,
    planned_column: &str,
    actual_column: &str,
) -> AppResult<(Option<f64>, Option<f64>)> {
    let planned_current =
        compute_planned_metric_value_at_now(pool, project_id, planned_column).await?;
    let planned_total = compute_planned_metric_total(pool, project_id, planned_column).await?;
    let actual_total = compute_actual_metric_total(pool, project_id, actual_column).await?;
    let planned_pct = percentage_of(planned_current, planned_total);
    let actual_pct = percentage_of(actual_total, planned_total);
    Ok((planned_pct, actual_pct))
}

async fn compute_planned_metric_value_at_now(
    pool: &SqlitePool,
    project_id: Uuid,
    column: &str,
) -> AppResult<Option<f64>> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT CAST({column} AS REAL)
         FROM project_plan
         WHERE {match_proj} AND {column} IS NOT NULL
         ORDER BY
            CASE WHEN datetime(date) <= datetime('now') THEN 0 ELSE 1 END,
            ABS(julianday(date) - julianday('now')) ASC
         LIMIT 1"
    );

    let value: Option<f64> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(pool)
        .await?
        .flatten();
    Ok(value)
}

async fn compute_planned_metric_total(
    pool: &SqlitePool,
    project_id: Uuid,
    column: &str,
) -> AppResult<Option<f64>> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT MAX(CAST({column} AS REAL))
         FROM project_plan
         WHERE {match_proj} AND {column} IS NOT NULL"
    );

    let value: Option<f64> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(pool)
        .await?;
    Ok(value)
}

async fn compute_actual_metric_total(
    pool: &SqlitePool,
    project_id: Uuid,
    column: &str,
) -> AppResult<Option<f64>> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT SUM(CAST({column} AS REAL))
         FROM work_logs
         WHERE {match_proj} AND deleted_at IS NULL"
    );
    let value: Option<f64> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(pool)
        .await?;
    Ok(value)
}

fn percentage_of(value: Option<f64>, total: Option<f64>) -> Option<f64> {
    let (Some(value), Some(total)) = (value, total) else {
        return None;
    };
    if total <= 0.0 {
        return None;
    }
    Some((value / total) * 100.0)
}

async fn compute_last_updated_at(pool: &SqlitePool, project_id: Uuid) -> AppResult<DateTime<Utc>> {
    let project_match = uuid_sql::match_uuid_clause("id");
    let task_match = uuid_sql::match_uuid_clause("project_id");
    let progress_match = uuid_sql::match_uuid_clause("project_id");
    let work_log_match = uuid_sql::match_uuid_clause("project_id");
    let plan_match = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT MAX(ts) FROM (
            SELECT updated_at AS ts FROM projects WHERE {}
            UNION ALL
            SELECT updated_at AS ts FROM tasks WHERE {} AND deleted_at IS NULL
            UNION ALL
            SELECT updated_at AS ts FROM task_progress WHERE {} AND deleted_at IS NULL
            UNION ALL
            SELECT updated_at AS ts FROM work_logs WHERE {} AND deleted_at IS NULL
            UNION ALL
            SELECT updated_at AS ts FROM project_plan WHERE {}
        )",
        project_match, task_match, progress_match, work_log_match, plan_match
    );
    let value: Option<String> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(pool)
        .await?;

    match value {
        Some(v) => parse_db_datetime(&v),
        None => Ok(utc_now()),
    }
}

async fn resolve_stage(
    pool: &SqlitePool,
    project_id: Uuid,
    metric: SCurveMetric,
    elapsed_time_pct: Option<f64>,
    planned_pct: Option<f64>,
    actual_pct: Option<f64>,
    variance_pct: Option<f64>,
    rule_50_70_pass: Option<bool>,
) -> AppResult<Option<SCurveStage>> {
    let metric_name = metric_to_str(metric);
    let project_match = uuid_sql::match_uuid_clause("rs.project_id");
    let sql_rule_set = format!(
        "SELECT rs.id
         FROM s_curve_stage_rule_sets rs
         WHERE rs.metric = ?
           AND rs.is_active = 1
           AND rs.deleted_at IS NULL
           AND (
                (rs.scope = 'project' AND rs.project_id IS NOT NULL AND {})
                OR
                (rs.scope = 'global' AND rs.project_id IS NULL)
           )
         ORDER BY
            CASE WHEN rs.scope = 'project' THEN 0 ELSE 1 END,
            rs.updated_at DESC
         LIMIT 1",
        project_match
    );
    let rule_set_id: Option<String> = sqlx::query_scalar(&sql_rule_set)
        .bind(metric_name)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(pool)
        .await?
        .flatten();

    let Some(rule_set_id) = rule_set_id else {
        return Ok(default_stage_from_variance(variance_pct));
    };

    let rows = sqlx::query(
        "SELECT
            stage,
            elapsed_from,
            elapsed_to,
            planned_from,
            planned_to,
            actual_from,
            actual_to,
            variance_from,
            variance_to,
            require_rule_50_70_pass
         FROM s_curve_stage_rules
         WHERE rule_set_id = ? AND deleted_at IS NULL
         ORDER BY priority ASC",
    )
    .bind(rule_set_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let stage_raw: String = row.try_get("stage")?;
        let stage = parse_stage(&stage_raw)?;
        let elapsed_from: Option<f64> = row.try_get("elapsed_from")?;
        let elapsed_to: Option<f64> = row.try_get("elapsed_to")?;
        let planned_from: Option<f64> = row.try_get("planned_from")?;
        let planned_to: Option<f64> = row.try_get("planned_to")?;
        let actual_from: Option<f64> = row.try_get("actual_from")?;
        let actual_to: Option<f64> = row.try_get("actual_to")?;
        let variance_from: Option<f64> = row.try_get("variance_from")?;
        let variance_to: Option<f64> = row.try_get("variance_to")?;
        let require_rule_50_70_pass_raw: Option<i64> = row.try_get("require_rule_50_70_pass")?;
        let require_rule_50_70_pass = require_rule_50_70_pass_raw.map(|v| v == 1);

        if !value_in_range(elapsed_time_pct, elapsed_from, elapsed_to) {
            continue;
        }
        if !value_in_range(planned_pct, planned_from, planned_to) {
            continue;
        }
        if !value_in_range(actual_pct, actual_from, actual_to) {
            continue;
        }
        if !value_in_range(variance_pct, variance_from, variance_to) {
            continue;
        }
        if let Some(required) = require_rule_50_70_pass {
            if rule_50_70_pass != Some(required) {
                continue;
            }
        }

        return Ok(Some(stage));
    }

    Ok(default_stage_from_variance(variance_pct))
}

fn value_in_range(value: Option<f64>, min: Option<f64>, max: Option<f64>) -> bool {
    if min.is_none() && max.is_none() {
        return true;
    }
    let Some(value) = value else {
        return false;
    };
    if let Some(min) = min {
        if value < min {
            return false;
        }
    }
    if let Some(max) = max {
        if value > max {
            return false;
        }
    }
    true
}

fn parse_stage(raw: &str) -> AppResult<SCurveStage> {
    match raw {
        "lag" => Ok(SCurveStage::Lag),
        "log" => Ok(SCurveStage::Log),
        "maturity" => Ok(SCurveStage::Maturity),
        "decline" => Ok(SCurveStage::Decline),
        other => Err(AppError::internal(format!(
            "invalid stage value in rules: {}",
            other
        ))),
    }
}

fn default_stage_from_variance(variance_pct: Option<f64>) -> Option<SCurveStage> {
    let variance = variance_pct?;
    if variance < -10.0 {
        Some(SCurveStage::Lag)
    } else if variance <= 0.0 {
        Some(SCurveStage::Log)
    } else if variance <= 10.0 {
        Some(SCurveStage::Maturity)
    } else {
        Some(SCurveStage::Decline)
    }
}

fn compute_rule_50_70_status(
    metric: SCurveMetric,
    elapsed_time_pct: Option<f64>,
    planned_pct: Option<f64>,
    actual_pct: Option<f64>,
) -> (Option<bool>, Rule5070Status) {
    let Some(elapsed) = elapsed_time_pct else {
        return (None, Rule5070Status::InsufficientElapsedTimeData);
    };
    let (Some(planned), Some(actual)) = (planned_pct, actual_pct) else {
        return (
            None,
            if metric == SCurveMetric::Progress {
                Rule5070Status::InsufficientProgressData
            } else {
                Rule5070Status::InsufficientMetricData
            },
        );
    };
    if elapsed < 50.0 {
        return (None, Rule5070Status::PreWindow);
    }
    let pass = actual >= planned;
    if elapsed <= 70.0 {
        return (
            Some(pass),
            if pass {
                Rule5070Status::Pass
            } else {
                Rule5070Status::Fail
            },
        );
    }
    (
        Some(pass),
        if pass {
            Rule5070Status::PostWindowPass
        } else {
            Rule5070Status::PostWindowFail
        },
    )
}

fn avg_option<I>(iter: I) -> Option<f64>
where
    I: Iterator<Item = Option<f64>>,
{
    let values: Vec<f64> = iter.flatten().collect();
    if values.is_empty() {
        None
    } else {
        Some(round2(values.iter().sum::<f64>() / values.len() as f64))
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn metric_to_str(metric: SCurveMetric) -> &'static str {
    match metric {
        SCurveMetric::Progress => "progress",
        SCurveMetric::Hours => "hours",
        SCurveMetric::Cost => "cost",
    }
}

fn default_cost_currency() -> String {
    let raw = std::env::var("SCURVE_COST_CURRENCY").unwrap_or_else(|_| "USD".to_string());
    let normalized = raw.trim().to_uppercase();
    if normalized.len() == 3 && normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        normalized
    } else {
        "USD".to_string()
    }
}

fn normalize_currency(currency: Option<String>) -> AppResult<Option<String>> {
    let Some(raw) = currency else {
        return Ok(None);
    };
    let normalized = raw.trim().to_uppercase();
    if normalized.is_empty() {
        return Ok(None);
    }
    if normalized.len() != 3 || !normalized.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::bad_request(
            "currency must be a 3-letter ISO code (e.g. USD)",
        ));
    }
    Ok(Some(normalized))
}

async fn resolve_cost_currency(pool: &SqlitePool, project_id: Uuid) -> AppResult<String> {
    let match_proj = uuid_sql::match_uuid_clause("project_id");
    let sql = format!(
        "SELECT currency
         FROM project_plan
         WHERE {match_proj}
           AND currency IS NOT NULL
           AND TRIM(currency) <> ''
         ORDER BY datetime(date) DESC, updated_at DESC
         LIMIT 1"
    );

    let found: Option<String> = sqlx::query_scalar(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(pool)
        .await?
        .flatten();

    if let Some(curr) = normalize_currency(found)? {
        Ok(curr)
    } else {
        Ok(default_cost_currency())
    }
}

fn parse_db_datetime(value: &str) -> AppResult<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        if let Some(naive) = date.and_hms_opt(0, 0, 0) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        }
    }
    Err(AppError::internal(format!("invalid datetime: {}", value)))
}
