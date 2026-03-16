use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::db::{row_parsers, uuid_sql};
use crate::errors::{AppError, AppResult};
use crate::models::task::{
    compute_task_schedule_status, managed_completed_at, managed_completed_at_is_backfilled, DbTask,
    Task, TaskExecutionStatus, TaskHealthStatus, TaskProgressMethod,
};
use crate::models::task_health::{EffectiveTaskHealthRule, EffectiveTaskHealthRuleSet};
use crate::models::task_progress_component::{DbTaskProgressComponent, TaskProgressComponent};
use crate::utils::utc_now;

const DEFAULT_EXPECTED_SOURCE: &str = "tasks.baseline_linear";
const DEFAULT_ACTUAL_SOURCE_MANUAL: &str = "tasks.progress";
const DEFAULT_ACTUAL_SOURCE_COMPONENTS: &str = "task_progress_components.weighted_completion";

pub fn task_select_columns(alias: &str) -> String {
    let id_case = uuid_sql::case_uuid(&format!("{alias}.id"));
    let project_case = uuid_sql::case_uuid(&format!("{alias}.project_id"));
    let assignee_case = uuid_sql::case_uuid(&format!("{alias}.assignee"));
    let parent_case = uuid_sql::case_uuid(&format!("{alias}.parent_id"));
    format!(
        "{id_case}, {project_case}, {alias}.title, {alias}.description, {alias}.status, \
         {alias}.progress_method, {alias}.blocked_flag, {alias}.blocked_reason, \
         {alias}.baseline_start_at, {alias}.baseline_end_at, {alias}.task_weight, \
         {alias}.due_date, {alias}.start_date, {alias}.end_date, {alias}.duration_days, \
         {assignee_case}, {parent_case}, {alias}.progress, {alias}.completed_at, \
         {alias}.completed_at_is_backfilled, {alias}.created_at, {alias}.updated_at, {alias}.deleted_at",
    )
}

pub async fn build_task_response(
    pool: &SqlitePool,
    project_id: Uuid,
    task: DbTask,
    now: DateTime<Utc>,
) -> AppResult<Task> {
    let tasks = build_task_responses(pool, project_id, vec![task], now).await?;
    tasks
        .into_iter()
        .next()
        .ok_or_else(|| AppError::internal("missing enriched task".to_string()))
}

pub async fn build_task_responses(
    pool: &SqlitePool,
    project_id: Uuid,
    tasks: Vec<DbTask>,
    now: DateTime<Utc>,
) -> AppResult<Vec<Task>> {
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let task_ids: Vec<Uuid> = tasks.iter().map(|task| task.id).collect();
    let component_actuals = load_component_actuals(pool, &task_ids).await?;
    let rules = load_effective_task_health_rules(pool, project_id).await?;

    tasks
        .into_iter()
        .map(|task| {
            let component_actual = component_actuals.get(&task.id).copied().flatten();
            Ok(build_task_response_from_parts(
                task,
                component_actual,
                &rules.rules,
                now,
            ))
        })
        .collect()
}

pub fn build_task_response_from_parts(
    task: DbTask,
    component_actual_progress_pct: Option<f64>,
    rules: &[EffectiveTaskHealthRule],
    now: DateTime<Utc>,
) -> Task {
    let expected_progress_pct =
        compute_expected_progress_pct(task.baseline_start_at, task.baseline_end_at, now)
            .map(round2);
    let (actual_progress_pct, actual_progress_source) = match task.progress_method {
        TaskProgressMethod::ManualPercentLegacy => (
            Some(round2(f64::from(task.progress))),
            Some(DEFAULT_ACTUAL_SOURCE_MANUAL.to_string()),
        ),
        TaskProgressMethod::WeightedComponents => (
            component_actual_progress_pct.map(round2),
            Some(DEFAULT_ACTUAL_SOURCE_COMPONENTS.to_string()),
        ),
    };
    let variance_pct = match (actual_progress_pct, expected_progress_pct) {
        (Some(actual), Some(expected)) => Some(round2(actual - expected)),
        _ => None,
    };
    let health_status = resolve_task_health_status(
        variance_pct,
        expected_progress_pct,
        actual_progress_pct,
        rules,
    );
    let execution_status = resolve_execution_status(task.blocked_flag, actual_progress_pct);
    let progress = match actual_progress_pct {
        Some(actual) => actual.round().clamp(0.0, 100.0) as i32,
        None => task.progress,
    };
    let schedule_status = compute_task_schedule_status(
        task.due_date,
        &task.status,
        progress,
        task.completed_at,
        task.updated_at,
        now,
    );

    Task {
        id: task.id,
        project_id: task.project_id,
        title: task.title,
        description: task.description,
        status: task.status,
        progress_method: task.progress_method,
        blocked_flag: task.blocked_flag,
        blocked_reason: task.blocked_reason,
        baseline_start_at: task.baseline_start_at,
        baseline_end_at: task.baseline_end_at,
        task_weight: round2(task.task_weight),
        due_date: task.due_date,
        start_date: task.start_date,
        end_date: task.end_date,
        duration_days: task.duration_days,
        assignee: task.assignee,
        parent_id: task.parent_id,
        progress,
        completed_at: task.completed_at,
        completed_at_is_backfilled: task.completed_at_is_backfilled,
        schedule_status,
        execution_status,
        expected_progress_pct,
        actual_progress_pct,
        variance_pct,
        health_status,
        expected_progress_source: expected_progress_pct
            .map(|_| DEFAULT_EXPECTED_SOURCE.to_string()),
        actual_progress_source,
        created_at: task.created_at,
        updated_at: task.updated_at,
        deleted_at: task.deleted_at,
    }
}

pub async fn load_component_actuals(
    pool: &SqlitePool,
    task_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, Option<f64>>> {
    if task_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders = std::iter::repeat_n("?", task_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let task_case = uuid_sql::expr_uuid("task_id");
    let sql = format!(
        "SELECT
            {task_case} AS task_id,
            CASE
                WHEN SUM(CASE WHEN deleted_at IS NULL THEN weight ELSE 0 END) > 0 THEN
                    SUM(CASE WHEN deleted_at IS NULL THEN weight * completion_pct ELSE 0 END)
                    / SUM(CASE WHEN deleted_at IS NULL THEN weight ELSE 0 END)
                ELSE NULL
            END AS actual_progress_pct
         FROM task_progress_components
         WHERE task_id IN ({placeholders})
         GROUP BY {task_case}"
    );

    let mut query = sqlx::query(&sql);
    for task_id in task_ids {
        query = query.bind(task_id.to_string());
    }

    let rows = query.fetch_all(pool).await?;
    let mut out = HashMap::with_capacity(rows.len());
    for row in rows {
        let task_id_s: String = row.try_get("task_id")?;
        let task_id = Uuid::parse_str(&task_id_s)
            .map_err(|e| AppError::internal(format!("invalid task component task_id: {}", e)))?;
        let actual_progress_pct: Option<f64> = row.try_get("actual_progress_pct")?;
        out.insert(task_id, actual_progress_pct);
    }

    Ok(out)
}

pub async fn load_task_progress_components(
    pool: &SqlitePool,
    task_id: Uuid,
) -> AppResult<Vec<TaskProgressComponent>> {
    let task_match = uuid_sql::match_uuid_clause("task_id");
    let sql = format!(
        "SELECT
            {id_case},
            {task_case},
            name,
            component_type,
            weight,
            completion_pct,
            planned_at,
            completed_at,
            sort_order,
            created_at,
            updated_at,
            deleted_at
         FROM task_progress_components
         WHERE {task_match} AND deleted_at IS NULL
         ORDER BY sort_order ASC, created_at ASC",
        id_case = uuid_sql::case_uuid("id"),
        task_case = uuid_sql::case_uuid("task_id"),
        task_match = task_match,
    );

    let rows = sqlx::query(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_all(pool)
        .await?;

    rows.into_iter()
        .map(|row| {
            let parsed: DbTaskProgressComponent =
                row_parsers::db_task_progress_component_from_row(&row)?;
            Ok(parsed.into())
        })
        .collect()
}

pub async fn load_effective_task_health_rules(
    pool: &SqlitePool,
    project_id: Uuid,
) -> AppResult<EffectiveTaskHealthRuleSet> {
    let project_match = uuid_sql::match_uuid_clause("trs.project_id");
    let sql = format!(
        "SELECT
            trs.id,
            trs.scope,
            {project_case} AS project_id,
            trs.updated_at
         FROM task_health_rule_sets trs
         WHERE trs.is_active = 1
           AND trs.deleted_at IS NULL
           AND (
                (trs.scope = 'project' AND trs.project_id IS NOT NULL AND {project_match})
                OR
                (trs.scope = 'global' AND trs.project_id IS NULL)
           )
         ORDER BY
            CASE WHEN trs.scope = 'project' THEN 0 ELSE 1 END,
            datetime(trs.updated_at) DESC
         LIMIT 1",
        project_case = uuid_sql::expr_uuid("trs.project_id"),
        project_match = project_match,
    );

    let rule_set_row = sqlx::query(&sql)
        .bind(project_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(pool)
        .await?;

    let Some(rule_set_row) = rule_set_row else {
        return Ok(default_rule_set());
    };

    let rule_set_id: String = rule_set_row.try_get("id")?;
    let scope: String = rule_set_row.try_get("scope")?;
    let project_id_s: Option<String> = rule_set_row.try_get("project_id")?;
    let updated_at_raw: String = rule_set_row.try_get("updated_at")?;

    let rows = sqlx::query(
        "SELECT health_status, variance_from, variance_to, priority
         FROM task_health_rules
         WHERE rule_set_id = ? AND deleted_at IS NULL
         ORDER BY priority ASC",
    )
    .bind(&rule_set_id)
    .fetch_all(pool)
    .await?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        let health_status_raw: String = row.try_get("health_status")?;
        let health_status = health_status_raw
            .parse::<TaskHealthStatus>()
            .map_err(AppError::internal)?;
        let variance_from: Option<f64> = row.try_get("variance_from")?;
        let variance_to: Option<f64> = row.try_get("variance_to")?;
        let priority: i32 = row.try_get("priority")?;
        rules.push(EffectiveTaskHealthRule {
            health_status,
            variance_from,
            variance_to,
            priority,
        });
    }

    if rules.is_empty() {
        return Ok(default_rule_set());
    }

    Ok(EffectiveTaskHealthRuleSet {
        scope,
        project_id: project_id_s
            .map(|value| Uuid::parse_str(&value))
            .transpose()
            .map_err(|e| AppError::internal(format!("invalid task health project_id: {}", e)))?,
        updated_at: row_parsers::parse_datetime_value(&updated_at_raw)?,
        rules,
    })
}

pub fn resolve_task_health_status(
    variance_pct: Option<f64>,
    expected_progress_pct: Option<f64>,
    actual_progress_pct: Option<f64>,
    rules: &[EffectiveTaskHealthRule],
) -> TaskHealthStatus {
    if expected_progress_pct.is_none() || actual_progress_pct.is_none() {
        return TaskHealthStatus::NeedsPlan;
    }
    let Some(variance_pct) = variance_pct else {
        return TaskHealthStatus::NeedsPlan;
    };

    for rule in rules {
        if task_value_in_range(variance_pct, rule.variance_from, rule.variance_to) {
            return rule.health_status;
        }
    }

    if variance_pct < -25.0 {
        TaskHealthStatus::Critical
    } else if variance_pct < -10.0 {
        TaskHealthStatus::AtRisk
    } else if variance_pct >= 10.0 {
        TaskHealthStatus::Ahead
    } else {
        TaskHealthStatus::OnTrack
    }
}

pub fn resolve_execution_status(
    blocked_flag: bool,
    actual_progress_pct: Option<f64>,
) -> TaskExecutionStatus {
    if blocked_flag {
        return TaskExecutionStatus::Blocked;
    }
    match actual_progress_pct {
        Some(actual) if actual >= 100.0 => TaskExecutionStatus::Completed,
        Some(actual) if actual <= 0.0 => TaskExecutionStatus::NotStarted,
        Some(_) => TaskExecutionStatus::InProgress,
        None => TaskExecutionStatus::NotStarted,
    }
}

pub fn compute_expected_progress_pct(
    baseline_start_at: Option<DateTime<Utc>>,
    baseline_end_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<f64> {
    let (Some(start), Some(end)) = (baseline_start_at, baseline_end_at) else {
        return None;
    };
    if end <= start {
        return None;
    }
    if now <= start {
        return Some(0.0);
    }
    if now >= end {
        return Some(100.0);
    }

    let total_ms = (end - start).num_milliseconds() as f64;
    if total_ms <= 0.0 {
        return None;
    }
    let elapsed_ms = (now - start).num_milliseconds() as f64;
    Some((elapsed_ms / total_ms) * 100.0)
}

pub async fn refresh_task_snapshot(pool: &SqlitePool, task_id: Uuid) -> AppResult<()> {
    let task_match = uuid_sql::match_uuid_clause("id");
    let sql = format!(
        "SELECT {columns}
         FROM tasks t
         WHERE {task_match} AND t.deleted_at IS NULL",
        columns = task_select_columns("t"),
        task_match = task_match,
    );
    let row = sqlx::query(&sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("task not found"))?;
    let task = row_parsers::db_task_from_row(&row)?;

    let next_progress = match task.progress_method {
        TaskProgressMethod::ManualPercentLegacy => latest_manual_progress(pool, task_id)
            .await?
            .unwrap_or(task.progress),
        TaskProgressMethod::WeightedComponents => load_component_actuals(pool, &[task_id])
            .await?
            .get(&task_id)
            .copied()
            .flatten()
            .map(|value| value.round().clamp(0.0, 100.0) as i32)
            .unwrap_or(0),
    };

    let now = utc_now();
    let next_completed_at =
        managed_completed_at(&task.status, next_progress, task.completed_at, now);
    let next_completed_at_is_backfilled = managed_completed_at_is_backfilled(
        task.completed_at,
        task.completed_at_is_backfilled,
        next_completed_at,
    );

    if next_progress == task.progress
        && next_completed_at == task.completed_at
        && next_completed_at_is_backfilled == task.completed_at_is_backfilled
    {
        return Ok(());
    }

    let update_sql = format!(
        "UPDATE tasks
         SET progress = ?, completed_at = ?, completed_at_is_backfilled = ?, updated_at = ?
         WHERE {task_match} AND deleted_at IS NULL",
        task_match = task_match,
    );
    sqlx::query(&update_sql)
        .bind(next_progress)
        .bind(next_completed_at)
        .bind(next_completed_at_is_backfilled)
        .bind(now)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn latest_manual_progress(pool: &SqlitePool, task_id: Uuid) -> AppResult<Option<i32>> {
    let latest_match = uuid_sql::match_uuid_clause("task_id");
    let latest_sql = format!(
        "SELECT progress
         FROM task_progress
         WHERE {latest_match} AND deleted_at IS NULL
         ORDER BY datetime(created_at) DESC, datetime(updated_at) DESC, rowid DESC
         LIMIT 1",
    );
    sqlx::query_scalar::<_, i32>(&latest_sql)
        .bind(task_id.to_string())
        .bind(task_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

fn task_value_in_range(value: f64, min: Option<f64>, max: Option<f64>) -> bool {
    if let Some(min) = min {
        if value < min {
            return false;
        }
    }
    if let Some(max) = max {
        if value >= max {
            return false;
        }
    }
    true
}

fn default_rule_set() -> EffectiveTaskHealthRuleSet {
    EffectiveTaskHealthRuleSet {
        scope: "global".to_string(),
        project_id: None,
        updated_at: utc_now(),
        rules: vec![
            EffectiveTaskHealthRule {
                health_status: TaskHealthStatus::Critical,
                variance_from: None,
                variance_to: Some(-25.0),
                priority: 1,
            },
            EffectiveTaskHealthRule {
                health_status: TaskHealthStatus::AtRisk,
                variance_from: Some(-25.0),
                variance_to: Some(-10.0),
                priority: 2,
            },
            EffectiveTaskHealthRule {
                health_status: TaskHealthStatus::OnTrack,
                variance_from: Some(-10.0),
                variance_to: Some(10.0),
                priority: 3,
            },
            EffectiveTaskHealthRule {
                health_status: TaskHealthStatus::Ahead,
                variance_from: Some(10.0),
                variance_to: None,
                priority: 4,
            },
        ],
    }
}

pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
