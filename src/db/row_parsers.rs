use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{
    dependency::DbTaskDependency, progress::DbProgress, project::DbProject,
    project_plan::DbProjectPlanPoint, task::DbTask, user::DbUser,
};

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, AppError> {
    let s = s.trim();

    // Try RFC3339 first (e.g. 2025-11-19T12:34:56Z)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try SQLite default timestamp format: "YYYY-MM-DD HH:MM:SS" (with optional fractional seconds)
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(Utc.from_utc_datetime(&naive));
    }

    // Try date-only format: "YYYY-MM-DD"
    if let Ok(naive_date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = naive_date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AppError::internal("invalid datetime: date out of range".to_string()))?;
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // Try unix epoch values represented as plain numbers.
    // Supports seconds and milliseconds.
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
                Ok(Some(parse_datetime(trimmed)?))
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
    if let Ok(v) = row.try_get::<Option<String>, _>(col) {
        return Ok(v);
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
    let created_at = parse_datetime(&created_at_s)?;
    let updated_at = parse_datetime(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime(&s)?)
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
    let id_s: String = row
        .try_get("id")
        .map_err(|e| AppError::internal(format!("missing id: {}", e)))?;
    let project_id_s: String = row
        .try_get("project_id")
        .map_err(|e| AppError::internal(format!("missing project_id: {}", e)))?;
    let title: String = row
        .try_get("title")
        .map_err(|e| AppError::internal(format!("missing title: {}", e)))?;
    let description: String = row
        .try_get("description")
        .map_err(|e| AppError::internal(format!("missing description: {}", e)))?;
    let status: String = row
        .try_get("status")
        .map_err(|e| AppError::internal(format!("missing status: {}", e)))?;
    let due_date_s: Option<String> = row
        .try_get("due_date")
        .map_err(|e| AppError::internal(format!("missing due_date: {}", e)))?;
    let start_date_s: Option<String> = row
        .try_get("start_date")
        .map_err(|e| AppError::internal(format!("missing start_date: {}", e)))?;
    let end_date_s: Option<String> = row
        .try_get("end_date")
        .map_err(|e| AppError::internal(format!("missing end_date: {}", e)))?;
    let duration_days: Option<i32> = row
        .try_get("duration_days")
        .map_err(|e| AppError::internal(format!("missing duration_days: {}", e)))?;
    let assignee_s: Option<String> = row
        .try_get("assignee")
        .map_err(|e| AppError::internal(format!("missing assignee: {}", e)))?;
    let parent_id_s: Option<String> = row
        .try_get("parent_id")
        .map_err(|e| AppError::internal(format!("missing parent_id: {}", e)))?;
    let progress: i32 = row
        .try_get("progress")
        .map_err(|e| AppError::internal(format!("missing progress: {}", e)))?;
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
    let project_id = Uuid::parse_str(&project_id_s)
        .map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?;
    let due_date = parse_opt_datetime(due_date_s)?;
    let start_date = parse_opt_datetime(start_date_s)?;
    let end_date = parse_opt_datetime(end_date_s)?;
    let assignee = match assignee_s {
        Some(s) => Some(
            Uuid::parse_str(&s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?,
        ),
        None => None,
    };
    let parent_id = match parent_id_s {
        Some(s) => Some(
            Uuid::parse_str(&s).map_err(|e| AppError::internal(format!("invalid uuid: {}", e)))?,
        ),
        None => None,
    };
    let created_at = parse_datetime(&created_at_s)?;
    let updated_at = parse_datetime(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime(&s)?)
    } else {
        None
    };

    Ok(DbTask {
        id,
        project_id,
        title,
        description,
        status,
        due_date,
        start_date,
        end_date,
        duration_days,
        assignee,
        parent_id,
        progress,
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
    let created_at = parse_datetime(&created_at_s)?;
    let updated_at = parse_datetime(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime(&s)?)
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
    let created_at = parse_datetime(&created_at_s)?;
    let updated_at = parse_datetime(&updated_at_s)?;
    let deleted_at = if let Some(s) = deleted_at_s {
        Some(parse_datetime(&s)?)
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
    let date = parse_datetime(&date_s)?;
    let created_at = parse_datetime(&created_at_s)?;
    let updated_at = parse_datetime(&updated_at_s)?;

    Ok(DbProjectPlanPoint {
        id,
        project_id,
        date,
        planned_progress,
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
    let created_at = parse_datetime(&created_at_s)?;

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
