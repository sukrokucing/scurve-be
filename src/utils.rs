use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{DateTime, Utc};
use rand_core::OsRng;

use crate::errors::AppError;

const MIN_PASSWORD_LENGTH: usize = 8;

pub fn hash_password(password: &str) -> Result<String, AppError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AppError::bad_request(format!(
            "password must be at least {} characters",
            MIN_PASSWORD_LENGTH
        )));
    }

    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| AppError::internal(format!("failed to hash password: {err}")))
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|err| AppError::internal(format!("invalid password hash: {err}")))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

/// Normalize a DateTime to midnight UTC (00:00:00) for consistent date comparisons.
/// This is used for task start_date and end_date to enable reliable milestone detection
/// in the frontend (where milestones are detected by comparing timestamps).
pub fn normalize_to_midnight(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
}

/// Round a floating-point value to two decimal places.
pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Parse a datetime string from SQLite (RFC 3339 or SQLite text format).
/// Canonical version — removes need for per-file duplicates.
pub fn parse_db_datetime(value: &str) -> crate::errors::AppResult<DateTime<Utc>> {
    crate::db::row_parsers::parse_datetime_value(value)
}
