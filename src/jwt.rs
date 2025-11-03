use std::sync::Arc;

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: Arc<Vec<u8>>,
    pub exp_hours: i64,
}

impl JwtConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let secret = std::env::var("JWT_SECRET").map_err(|_| AppError::configuration("JWT_SECRET not set"))?;
        let exp_hours = std::env::var("JWT_EXP_HOURS")
            .map(|val| val.parse::<i64>())
            .unwrap_or(Ok(24))
            .map_err(|_| AppError::configuration("JWT_EXP_HOURS must be a valid integer"))?;

        Ok(Self {
            secret: Arc::new(secret.into_bytes()),
            exp_hours,
        })
    }

    pub fn encode(&self, user_id: Uuid) -> Result<String, AppError> {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let exp = now + Duration::hours(self.exp_hours);

        let claims = Claims {
            sub: user_id,
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        jsonwebtoken::encode(&Header::default(), &claims, &EncodingKey::from_secret(&self.secret))
            .map_err(|err| AppError::token(err.to_string()))
    }

    pub fn decode(&self, token: &str) -> Result<Claims, AppError> {
        let mut validation = Validation::default();
        validation.validate_exp = true;

        jsonwebtoken::decode::<Claims>(token, &DecodingKey::from_secret(&self.secret), &validation)
            .map(|data| data.claims)
            .map_err(|err| AppError::token(err.to_string()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::unauthorized("Authorization header missing"))?;

        let claims = state.jwt.decode(token)?;

        Ok(AuthUser {
            user_id: claims.sub,
        })
    }
}
