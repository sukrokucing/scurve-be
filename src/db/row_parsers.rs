use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{
    dependency::DbTaskDependency,
    progress::DbProgress,
    project::DbProject,
    project_plan::DbProjectPlanPoint,
    task::{DbTask, TaskProgressMethod},
    task_progress_component::DbTaskProgressComponent,
    user::DbUser,
};

fn is_missing_column(err: &sqlx::Error, col: &str) -> bool {
    matches!(err, sqlx::Error::ColumnNotFound(name) if name == col)
}

pub fn parse_datetime_value(s: &str) -> Result<DateTime<Utc>, AppError> {
    let s = s.trim();

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(Utc.from_utc_datetime(&naive));
    }

    if let Ok(naive_date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = naive_date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AppError::internal("invalid datetime: date out of range".to_string()))?;
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    if let Ok(epoch) = s.parse::<i64>() {
        let (secs, nanos) = if epoch.abs() >= 1_000_000_000_000 {
            let secs = epoch.div_euclid(1_000);
            let ms = epoch.rem_euclid(1_000) as u32;
            (secs, ms * 1_000_000)
        } else {
            (epoch, 0)
        };
        if let Some(dt) = Utc.timestamp_opt(secs, nanos).single() {
            return Ok(dt);
        }
    }

    Err(AppError::internal(format!("invalid datetime: {}", s)))
}

fn parse_opt_datetime(s: Option<String>) -> Result<Option<DateTime<Utc>>, AppError> {
    match s {
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(parse_datetime_value(trimmed)?))
            }
        }
        None => Ok(None),
    }
}

fn get_required_text(row: &SqliteRow, col: &str) -> Result<String, AppError> {
    if let Ok(v) = row.try_get::<String, _>(col) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(col) {
        if let Ok(s) = String::from_utf8(v.clone()) {
            return Ok(s);
        }
        return Ok(String::from_utf8_lossy(&v).to_string());
    }

    Err(AppError::internal(format!(
        "missing {}: decode failed",
        col
    )))
}

fn get_optional_text(row: &SqliteRow, col: &str) -> Result<Option<String>, AppError> {
    match row.try_get::<Option<String>, _>(col) {
        Ok(v) => return Ok(v),
        Err(err) if is_missing_column(&err, col) => return Ok(None),
        Err(_) => {}
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return Ok(v.map(|x| x.to_string()));
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
        return Ok(v.map(|x| x.to_string()));
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(col) {
        return Ok(v.map(|bytes| String::from_utf8_lossy(&bytes).to_string()));
    }

    Err(AppError::internal(format!(
        "missing {}: decode failed",
        col
    )))
}

fn get_optional_f64(row: &SqliteRow, col: &str) -> Result<Option<f64>, AppError> {
    match row.try_get::<Option<f64>, _>(col) {
        Ok(v) => return Ok(v),
        Err(err) if is_missing_column(&err, col) => return Ok(None),
        Err(_) => {}
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return Ok(v.map(|x| x as f64));
    }
    if let Ok(v) = row.try_get::<Option<String>, _>(col) {
        return match v {
            Some(raw) => raw
                .trim()
                .parse::<f64>()
                .map(Some)
                .map_err(|e| AppError::internal(format!("invalid {}: {}", col, e))),
            None => Ok(None),
        };
    }

    Err(AppError::internal(format!(
        "missing {}: decode failed",
        col
    )))
}

fn get_required_i32(row: &SqliteRow, col: &str) -> Result<i32, AppError> {
    if let Ok(v) = row.try_get::<i32, _>(col) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return i32::try_from(v)
            .map_err(|_| AppError::internal(format!("invalid {} out of range: {}", col, v)));
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        if v.fract() == 0.0 {
            let as_i64 = v as i64;
            return i32::try_from(as_i64)
                .map_err(|_| AppError::internal(format!("invalid {} out of range: {}", col, v)));
        }
        return Err(AppError::internal(format!(
            "invalid {} not integer: {}",
            col, v
        )));
    }
    if let Ok(v) = row.try_get::<String, _>(col) {
        return v
            .trim()
            .parse::<i32>()
            .map_err(|e| AppError::internal(format!("invalid {}: {}", col, e)));
    }

    Err(AppError::internal(format!(
        "missing {}: decode failed",
        col
    )))
}

pub fn db_progress_from_row(row: &SqliteRow) -> Result<DbProgress, AppError> {
    let id_s = get_required_text(row, "id")?;
    let project_id_s = get_required_text(row, "project_id")?;
    let task_id_s = get_required_text(row, "task_id")?;
    let progress_val = get_required_i32(row, "progress")?;
    let note = get_optional_text(row, "note")?;
    let created_at_s = get_required_text(row, "created_at")?;
    let updated_at_s = get_required_text(row, "updated_at")?;
    let deleted_at_s = get_optional_text(row, "deleted_at")?;

    let id =
        Uuid::parse_str(&id_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let project_id = Uuid::parse_str(&project_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let task_id = Uuid::parse_str(&task_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let created_at = parse_datetime_value(&created_at_s)?;
    let updated_at = parse_datetime_value(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime_value(&s)?)
    } else {
        None
    };

    Ok(DbProgress {
        id,
        project_id,
        task_id,
        progress: progress_val,
        note,
        created_at,
        updated_at,
        deleted_at,
    })
}

pub fn db_task_from_row(row: &SqliteRow) -> Result<DbTask, AppError> {
    let id = Uuid::parse_str(&get_required_text(row, "id")?)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let project_id = Uuid::parse_str(&get_required_text(row, "project_id")?)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let title = get_required_text(row, "title")?;
    let description = get_required_text(row, "description")?;
    let status = get_required_text(row, "status")?;
    let progress_method = get_optional_text(row, "progress_method")?
        .unwrap_or_else(|| "manual_percent_legacy".to_string())
        .parse::<TaskProgressMethod>()
        .map_err(AppError::internal)?;
    let blocked_flag = row.try_get::<i64, _>("blocked_flag").unwrap_or(0) != 0;
    let blocked_reason = get_optional_text(row, "blocked_reason")?;
    let baseline_start_at = parse_opt_datetime(get_optional_text(row, "baseline_start_at")?)?;
    let baseline_end_at = parse_opt_datetime(get_optional_text(row, "baseline_end_at")?)?;
    let task_weight = get_optional_f64(row, "task_weight")?.unwrap_or(1.0);
    let due_date = parse_opt_datetime(get_optional_text(row, "due_date")?)?;
    let start_date = parse_opt_datetime(get_optional_text(row, "start_date")?)?;
    let end_date = parse_opt_datetime(get_optional_text(row, "end_date")?)?;
    let duration_days: Option<i32> = row
        .try_get("duration_days")
        .map_err(|e| AppError::internal(format!("missing duration_days: {}", e)))?;
    let assignee = get_optional_text(row, "assignee")?
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let parent_id = get_optional_text(row, "parent_id")?
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let progress = get_required_i32(row, "progress")?;
    let completed_at = parse_opt_datetime(get_optional_text(row, "completed_at")?)?;
    let completed_at_is_backfilled = row
        .try_get::<i64, _>("completed_at_is_backfilled")
        .unwrap_or(0)
        != 0;
    let created_at = parse_datetime_value(&get_required_text(row, "created_at")?)?;
    let updated_at = parse_datetime_value(&get_required_text(row, "updated_at")?)?;
    let deleted_at = parse_opt_datetime(get_optional_text(row, "deleted_at")?)?;

    Ok(DbTask {
        id,
        project_id,
        title,
        description,
        status,
        progress_method,
        blocked_flag,
        blocked_reason,
        baseline_start_at,
        baseline_end_at,
        task_weight,
        due_date,
        start_date,
        end_date,
        duration_days,
        assignee,
        parent_id,
        progress,
        completed_at,
        completed_at_is_backfilled,
        created_at,
        updated_at,
        deleted_at,
    })
}

pub fn db_task_progress_component_from_row(
    row: &SqliteRow,
) -> Result<DbTaskProgressComponent, AppError> {
    let id = Uuid::parse_str(&get_required_text(row, "id")?)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let task_id = Uuid::parse_str(&get_required_text(row, "task_id")?)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let name = get_required_text(row, "name")?;
    let component_type = get_required_text(row, "component_type")?;
    let weight = get_optional_f64(row, "weight")?
        .ok_or_else(|| AppError::internal("missing weight".to_string()))?;
    let completion_pct = get_optional_f64(row, "completion_pct")?
        .ok_or_else(|| AppError::internal("missing completion_pct".to_string()))?;
    let planned_at = parse_opt_datetime(get_optional_text(row, "planned_at")?)?;
    let completed_at = parse_opt_datetime(get_optional_text(row, "completed_at")?)?;
    let sort_order = get_required_i32(row, "sort_order")?;
    let created_at = parse_datetime_value(&get_required_text(row, "created_at")?)?;
    let updated_at = parse_datetime_value(&get_required_text(row, "updated_at")?)?;
    let deleted_at = parse_opt_datetime(get_optional_text(row, "deleted_at")?)?;

    Ok(DbTaskProgressComponent {
        id,
        task_id,
        name,
        component_type,
        weight,
        completion_pct,
        planned_at,
        completed_at,
        sort_order,
        created_at,
        updated_at,
        deleted_at,
    })
}

pub fn db_project_from_row(row: &SqliteRow) -> Result<DbProject, AppError> {
    let id_s: String = row
        .try_get("id")
        .map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
    let user_id_s: String = row
        .try_get("user_id")
        .map_err(|e| AppError::internal(format!("missing user_id: {}", e)))?;
    let name: String = row
        .try_get("name")
        .map_err(|e| AppError::internal(format!("missing name: {}", e)))?;
    let description: Option<String> = row
        .try_get("description")
        .map_err(|e| AppError::internal(format!("missing description: {}", e)))?;
    let theme_color: String = row
        .try_get("theme_color")
        .map_err(|e| AppError::internal(format!("missing theme_color: {}", e)))?;
    let created_at_s: String = row
        .try_get("created_at")
        .map_err(|e| AppError::internal(format!("missing created_at: {}", e)))?;
    let updated_at_s: String = row
        .try_get("updated_at")
        .map_err(|e| AppError::internal(format!("missing updated_at: {}", e)))?;
    let deleted_at_s: Option<String> = row
        .try_get("deleted_at")
        .map_err(|e| AppError::internal(format!("missing deleted_at: {}", e)))?;

    let id =
        Uuid::parse_str(&id_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let user_id = Uuid::parse_str(&user_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let created_at = parse_datetime_value(&created_at_s)?;
    let updated_at = parse_datetime_value(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime_value(&s)?)
    } else {
        None
    };

    Ok(DbProject {
        id,
        user_id,
        name,
        description,
        theme_color,
        created_at,
        updated_at,
        deleted_at,
    })
}

pub fn db_user_from_row(row: &SqliteRow) -> Result<DbUser, AppError> {
    let id_s: String = row
        .try_get("id")
        .map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
    let name: String = row
        .try_get("name")
        .map_err(|e| AppError::internal(format!("missing name: {}", e)))?;
    let email: String = row
        .try_get("email")
        .map_err(|e| AppError::internal(format!("missing email: {}", e)))?;
    let password_hash: String = row
        .try_get("password_hash")
        .map_err(|e| AppError::internal(format!("missing password_hash: {}", e)))?;
    let provider: String = row
        .try_get("provider")
        .map_err(|e| AppError::internal(format!("missing provider: {}", e)))?;
    let provider_id: Option<String> = row
        .try_get("provider_id")
        .map_err(|e| AppError::internal(format!("missing provider_id: {}", e)))?;
    let created_at_s: String = row
        .try_get("created_at")
        .map_err(|e| AppError::internal(format!("missing created_at: {}", e)))?;
    let updated_at_s: String = row
        .try_get("updated_at")
        .map_err(|e| AppError::internal(format!("missing updated_at: {}", e)))?;
    let deleted_at_s: Option<String> = row
        .try_get("deleted_at")
        .map_err(|e| AppError::internal(format!("missing deleted_at: {}", e)))?;

    let id =
        Uuid::parse_str(&id_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let created_at = parse_datetime_value(&created_at_s)?;
    let updated_at = parse_datetime_value(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime_value(&s)?)
    } else {
        None
    };

    Ok(DbUser {
        id,
        name,
        email,
        password_hash,
        provider,
        provider_id,
        created_at,
        updated_at,
        deleted_at,
    })
}

pub fn db_project_plan_point_from_row(row: &SqliteRow) -> Result<DbProjectPlanPoint, AppError> {
    let id_s: String = row
        .try_get("id")
        .map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
    let project_id_s: String = row
        .try_get("project_id")
        .map_err(|e| AppError::internal(format!("missing project_id: {}", e)))?;
    let date_s: String = row
        .try_get("date")
        .map_err(|e| AppError::internal(format!("missing date: {}", e)))?;
    let planned_progress: i32 = row
        .try_get("planned_progress")
        .map_err(|e| AppError::internal(format!("missing planned_progress: {}", e)))?;
    let planned_hours = get_optional_f64(row, "planned_hours")?;
    let planned_cost = get_optional_f64(row, "planned_cost")?;
    let currency = get_optional_text(row, "currency")?;
    let created_at_s: String = row
        .try_get("created_at")
        .map_err(|e| AppError::internal(format!("missing created_at: {}", e)))?;
    let updated_at_s: String = row
        .try_get("updated_at")
        .map_err(|e| AppError::internal(format!("missing updated_at: {}", e)))?;

    let id =
        Uuid::parse_str(&id_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let project_id = Uuid::parse_str(&project_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let date = parse_datetime_value(&date_s)?;
    let created_at = parse_datetime_value(&created_at_s)?;
    let updated_at = parse_datetime_value(&updated_at_s)?;

    Ok(DbProjectPlanPoint {
        id,
        project_id,
        date,
        planned_progress,
        planned_hours,
        planned_cost,
        currency,
        created_at,
        updated_at,
    })
}

pub fn db_task_dependency_from_row(row: &SqliteRow) -> Result<DbTaskDependency, AppError> {
    let id_s: String = row
        .try_get("id")
        .map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
    let source_task_id_s: String = row
        .try_get("source_task_id")
        .map_err(|e| AppError::internal(format!("missing source_task_id: {}", e)))?;
    let target_task_id_s: String = row
        .try_get("target_task_id")
        .map_err(|e| AppError::internal(format!("missing target_task_id: {}", e)))?;
    let type_: String = row
        .try_get("type")
        .map_err(|e| AppError::internal(format!("missing type: {}", e)))?;
    let created_at_s: String = row
        .try_get("created_at")
        .map_err(|e| AppError::internal(format!("missing created_at: {}", e)))?;

    let id =
        Uuid::parse_str(&id_s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let source_task_id = Uuid::parse_str(&source_task_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let target_task_id = Uuid::parse_str(&target_task_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let created_at = parse_datetime_value(&created_at_s)?;

    Ok(DbTaskDependency {
        id,
        source_task_id,
        target_task_id,
        type_,
        created_at,
    })
}

#[cfg(test)]
mod tests {}
