use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const DEFAULT_TIMEZONE: &str = "Asia/Jakarta";
pub const DEFAULT_LOCALE: &str = "id-ID";
pub const DEFAULT_HOUR_CYCLE: i32 = 24;
pub const DEFAULT_CURRENCY: &str = "IDR";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPreferences {
    #[schema(example = "Asia/Jakarta")]
    pub timezone: String,
    #[schema(example = "id-ID")]
    pub locale: String,
    #[schema(example = 24)]
    pub hour_cycle: i32,
    #[schema(example = "IDR")]
    pub currency: String,
    pub updated_at: DateTime<Utc>,
}

/// Preferences embedded inside the user object on /auth/me and /auth/login.
/// Omits `updated_at` — clients that need it should call GET /auth/me/preferences.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPreferencesEmbed {
    pub timezone: String,
    pub locale: String,
    pub hour_cycle: i32,
    pub currency: String,
}

impl From<UserPreferences> for UserPreferencesEmbed {
    fn from(p: UserPreferences) -> Self {
        Self {
            timezone: p.timezone,
            locale: p.locale,
            hour_cycle: p.hour_cycle,
            currency: p.currency,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UserPreferencesUpdate {
    #[schema(example = "Asia/Jakarta")]
    pub timezone: String,
    #[schema(example = "id-ID")]
    pub locale: String,
    #[schema(example = 24)]
    pub hour_cycle: i32,
    #[schema(example = "IDR")]
    pub currency: String,
}

