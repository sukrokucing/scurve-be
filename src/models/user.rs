use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub provider: String,
    pub provider_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl crate::events::Loggable for User {
    fn entity_type() -> &'static str { "user" }
    fn subject_id(&self) -> Uuid { self.id }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub provider: String,
    pub provider_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<DbUser> for User {
    type Error = AppError;

    fn try_from(value: DbUser) -> Result<Self, Self::Error> {
        Ok(User {
            id: value.id,
            name: value.name,
            email: value.email,
            provider: value.provider,
            provider_id: value.provider_id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            deleted_at: value.deleted_at,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    #[schema(example = "Ada Lovelace")]
    pub name: String,
    #[schema(example = "ada@example.com")]
    pub email: String,
    #[schema(example = "S3cureP@ssw0rd")]
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "ada@example.com")]
    pub email: String,
    #[schema(example = "S3cureP@ssw0rd")]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}
