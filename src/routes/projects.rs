use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;
use sqlx::Row;
use uuid::Uuid;
use crate::db::{uuid_sql, row_parsers};

use crate::app::AppState;
use crate::errors::{AppError, AppResult};
use crate::jwt::AuthUser;
use crate::models::project::{DbProject, Project, ProjectCreateRequest, ProjectUpdateRequest};
use crate::models::project_plan::{DbProjectPlanPoint, ProjectPlanPoint};
use serde::Serialize;
use utoipa::ToSchema;
use crate::utils::utc_now;

const DEFAULT_THEME: &str = "#3498db";

#[utoipa::path(
    get,
    path = "/projects",
    tag = "Projects",
    responses((status = 200, description = "List projects", body = [Project]))
)]
pub async fn list_projects(State(state): State<AppState>, auth: AuthUser) -> AppResult<Json<Vec<Project>>> {
    // Try the simple, direct SELECT first (fast path). If decoding fails due to mixed UUID storage
    // (BLOB vs TEXT), fall back to a query that returns text UUIDs and map manually.
    let simple = sqlx::query_as::<_, DbProject>(
        "SELECT id, user_id, name, description, theme_color, created_at, updated_at, deleted_at FROM projects WHERE user_id = ? AND deleted_at IS NULL ORDER BY created_at DESC",
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await;

    let projects: Vec<DbProject> = match simple {
        Ok(rows) => rows,
        Err(_) => {
            // Fallback: return textified id/user_id and parse manually
            let id_case = uuid_sql::case_uuid("id");
            let user_case = uuid_sql::case_uuid("user_id");
            let match_user = uuid_sql::match_uuid_clause("user_id");
            let sql = format!(
                "SELECT {} , {} , name, description, theme_color, created_at, updated_at, deleted_at FROM projects WHERE {} AND deleted_at IS NULL ORDER BY created_at DESC",
                id_case, user_case, match_user
            );

            let rows = sqlx::query(&sql)
                .bind(auth.user_id.to_string())
                .bind(auth.user_id.to_string())
                .fetch_all(&state.pool)
                .await?;

            // Map each row from sqlx::Row to DbProject by extracting columns and parsing types
            let mut parsed = Vec::with_capacity(rows.len());
            for row in rows {
                parsed.push(row_parsers::db_project_from_row(&row)?);
            }

            parsed
        }
    };

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
    responses((status = 201, description = "Project created", body = Project))
)]
pub async fn create_project(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ProjectCreateRequest>,
) -> AppResult<(StatusCode, Json<Project>)> {
    let now = utc_now();
    let project_id = Uuid::new_v4();
    let theme_color = payload.theme_color.clone().unwrap_or_else(|| DEFAULT_THEME.to_string());

    sqlx::query(
        "INSERT INTO projects (id, user_id, name, description, theme_color, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(project_id)
    .bind(auth.user_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(&theme_color)
    .bind(now)
    .bind(now)
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
    responses((status = 200, description = "Project detail", body = Project))
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
    responses((status = 200, description = "Project updated", body = Project))
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

    sqlx::query(
        "UPDATE projects SET name = ?, description = ?, theme_color = ?, updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&project.name)
    .bind(&project.description)
    .bind(&project.theme_color)
    .bind(now)
    .bind(project.id)
    .bind(auth.user_id)
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
    responses((status = 204, description = "Project soft deleted"))
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
    let affected = sqlx::query("UPDATE projects SET deleted_at = ?, updated_at = ? WHERE id = ? AND user_id = ? AND deleted_at IS NULL")
        .bind(now)
        .bind(now)
        .bind(id)
        .bind(auth.user_id)
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
    // Try the simple (original) path first. If row conversion fails (e.g., mixed UUID storage blob/text),
    // fall back to a query that handles both blob and text UUID representations.
    let simple = sqlx::query_as::<_, DbProject>(
        "SELECT id, user_id, name, description, theme_color, created_at, updated_at, deleted_at FROM projects WHERE id = ? AND user_id = ? AND deleted_at IS NULL",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await;

    match simple {
        Ok(Some(row)) => Ok(row),
        Ok(None) => Err(AppError::not_found("project not found")),
        Err(_) => {
            // Fallback: handle mixed storage where UUIDs may be stored as BLOB (raw 16 bytes) or TEXT.
            let id_case = uuid_sql::case_uuid("id");
            let user_case = uuid_sql::case_uuid("user_id");
            let match_id = uuid_sql::match_uuid_clause("id");
            let match_user = uuid_sql::match_uuid_clause("user_id");

            let sql = format!(
                "SELECT {} , {} , name, description, theme_color, created_at, updated_at, deleted_at FROM projects WHERE {} AND {} AND deleted_at IS NULL",
                id_case, user_case, match_id, match_user
            );

            let fallback = sqlx::query(&sql)
                .bind(project_id.to_string())
                .bind(project_id.to_string())
                .bind(user_id.to_string())
                .bind(user_id.to_string())
                .fetch_optional(pool)
                .await?;

            if let Some(row) = fallback {
                return Ok(row_parsers::db_project_from_row(&row)?);
            }

            Err(AppError::not_found("project not found"))
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActualPoint {
    pub date: String,
    pub actual: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DashboardResponse {
    pub project: Project,
    pub plan: Vec<ProjectPlanPoint>,
    pub actual: Vec<ActualPoint>,
}

#[utoipa::path(
    get,
    path = "/projects/{id}/dashboard",
    tag = "Projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Project dashboard", body = DashboardResponse))
)]
pub async fn get_project_dashboard(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DashboardResponse>> {
    // ensure project exists and belongs to user
    let db_project = fetch_project(&state.pool, auth.user_id, id).await?;
    let project: Project = db_project.try_into()?;

    // fetch planned points (try fast-path mapping then fallback to tolerant parsing)
    let simple = sqlx::query_as::<_, DbProjectPlanPoint>(
        "SELECT id, project_id, date, planned_progress, created_at, updated_at FROM project_plan WHERE project_id = ? ORDER BY date ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await;

    let plan_rows: Vec<DbProjectPlanPoint> = match simple {
        Ok(r) => r,
        Err(_) => {
            let id_case = uuid_sql::case_uuid("id");
            let proj_case = uuid_sql::case_uuid("project_id");
            let sql = format!(
                "SELECT {} , {} , date, planned_progress, created_at, updated_at FROM project_plan WHERE project_id = ? ORDER BY date ASC",
                id_case, proj_case
            );

            let rows = sqlx::query(&sql)
                .bind(id.to_string())
                .fetch_all(&state.pool)
                .await?;

            let mut parsed = Vec::with_capacity(rows.len());
            for row in rows {
                parsed.push(row_parsers::db_project_plan_point_from_row(&row)?);
            }

            parsed
        }
    };

    let plan: Vec<ProjectPlanPoint> = plan_rows
        .into_iter()
        .map(ProjectPlanPoint::try_from)
        .collect::<Result<_, _>>()?;

    // fetch actual aggregated progress per day
    let actual_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT DATE(created_at) as date, CAST(ROUND(AVG(progress)) AS INTEGER) as actual FROM task_progress WHERE project_id = ? AND deleted_at IS NULL GROUP BY DATE(created_at) ORDER BY DATE(created_at) ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let actual: Vec<ActualPoint> = actual_rows
        .into_iter()
        .map(|(date, actual)| ActualPoint { date, actual: actual as i32 })
        .collect();

    let resp = DashboardResponse { project, plan, actual };

    Ok(Json(resp))
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
    responses((status = 200, description = "Critical path task ids", body = CriticalPathResponse))
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
        let id_s: String = row.try_get("id").map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
        let dur: i64 = row.try_get("duration_days").map_err(|e| AppError::internal(format!("missing duration_days: {}", e)))?;
        let tu = Uuid::parse_str(&id_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
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
        let src_s: String = row.try_get("source_task_id").map_err(|e| AppError::internal(format!("missing source_task_id: {}", e)))?;
        let tgt_s: String = row.try_get("target_task_id").map_err(|e| AppError::internal(format!("missing target_task_id: {}", e)))?;
        let src = Uuid::parse_str(&src_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
        let tgt = Uuid::parse_str(&tgt_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
        if !nodes.contains(&src) || !nodes.contains(&tgt) { continue; }
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
                if let Some(e) = indeg.get_mut(&m) { *e -= 1; if *e == 0 { q.push_back(m); } }
            }
        }
    }

    if topo.len() != nodes.len() {
        return Err(AppError::internal("dependency graph is not a DAG".to_string()));
    }

    // DP for longest path (by duration). Initialize best[node] = duration[node]
    let mut best: HashMap<Uuid, i64> = HashMap::new();
    let mut prev: HashMap<Uuid, Option<Uuid>> = HashMap::new();
    for &n in topo.iter() { best.insert(n, durations.get(&n).cloned().unwrap_or(0) as i64); prev.insert(n, None); }

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
        if val > max_val { max_val = val; max_node = Some(n); }
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
    responses((status = 200, description = "Project plan updated", body = [ProjectPlanPoint]))
)]
pub async fn update_project_plan(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<Vec<crate::models::project_plan::ProjectPlanCreateRequest>>,
) -> AppResult<Json<Vec<ProjectPlanPoint>>> {
    // ensure project exists and belongs to user
    let owner = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM projects WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let owner = owner.ok_or_else(|| AppError::not_found("project not found"))?;
    if owner != auth.user_id {
        return Err(AppError::forbidden("not allowed to access this project"));
    }

    let mut tx = state.pool.begin().await?;
    let now = utc_now();

    // 1. Clear existing plan
    sqlx::query("DELETE FROM project_plan WHERE project_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // 2. Insert new points
    let mut inserted_ids = Vec::new();
    for point in payload {
        if point.planned_progress < 0 || point.planned_progress > 100 {
             return Err(AppError::bad_request("planned_progress must be between 0 and 100"));
        }

        let pid = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO project_plan (id, project_id, date, planned_progress, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(pid)
        .bind(id)
        .bind(point.date)
        .bind(point.planned_progress)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        inserted_ids.push(pid);
    }

    tx.commit().await?;

    // 3. Fetch and return new plan
    let simple = sqlx::query_as::<_, DbProjectPlanPoint>(
        "SELECT id, project_id, date, planned_progress, created_at, updated_at FROM project_plan WHERE project_id = ? ORDER BY date ASC",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await;

    let plan_rows: Vec<DbProjectPlanPoint> = match simple {
        Ok(r) => r,
        Err(_) => {
             // Fallback for UUID text/blob mismatch if necessary, though we just inserted them so it should be consistent with driver default.
             // But to be safe and consistent with get_dashboard:
            let id_case = uuid_sql::case_uuid("id");
            let proj_case = uuid_sql::case_uuid("project_id");
            let sql = format!(
                "SELECT {} , {} , date, planned_progress, created_at, updated_at FROM project_plan WHERE project_id = ? ORDER BY date ASC",
                id_case, proj_case
            );
            let rows = sqlx::query(&sql)
                .bind(id.to_string())
                .fetch_all(&state.pool)
                .await?;
             let mut parsed = Vec::with_capacity(rows.len());
             for row in rows {
                 parsed.push(row_parsers::db_project_plan_point_from_row(&row)?);
             }
             parsed
        }
    };

    let plan: Vec<ProjectPlanPoint> = plan_rows
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
    responses((status = 204, description = "Project plan cleared"))
)]
pub async fn clear_project_plan(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // ensure project exists and belongs to user
    let owner = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM projects WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let owner = owner.ok_or_else(|| AppError::not_found("project not found"))?;
    if owner != auth.user_id {
        return Err(AppError::forbidden("not allowed to access this project"));
    }

    sqlx::query("DELETE FROM project_plan WHERE project_id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
