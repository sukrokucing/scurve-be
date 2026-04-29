use serde::Serialize;
use utoipa::ToSchema;

/// Internal DB row — not exposed via API.
#[derive(Debug, sqlx::FromRow)]
pub struct DbMenu {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub route: String,
    pub section: String,
    pub priority: i32,
    /// JSON array stored as TEXT, e.g. `["sidebar","search"]`
    pub surfaces: String,
    pub icon: String,
    /// JSON array stored as TEXT, e.g. `["home","overview"]`
    pub keywords: String,
    pub required_permission: Option<String>,
    // `hidden` is intentionally not selected — the SQL query already filters `WHERE hidden = 0`.
    /// SQLite stores as 0/1 INTEGER; PostgreSQL as BOOLEAN — SQLx maps both to `bool`.
    pub disabled: bool,
}

/// A single navigation menu item visible to the current user.
#[derive(Debug, Serialize, ToSchema)]
pub struct MenuResponse {
    /// Stable slug identifier (e.g. `"dashboard"`, `"settings-roles"`).
    pub id: String,
    /// Human-readable label shown in the UI.
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Client-side route path (e.g. `"/"`, `"/projects"`).
    pub route: String,
    /// Grouping: `"main"` | `"settings"`.
    pub section: String,
    /// Sort order within section — lower = first.
    pub priority: i32,
    /// UI surfaces where this item appears (e.g. `["sidebar","bottom-nav","search"]`).
    pub surfaces: Vec<String>,
    /// Lucide icon name (e.g. `"LayoutDashboard"`).
    pub icon: String,
    /// Keywords used for command-palette / search matching.
    pub keywords: Vec<String>,
}

/// Filtered navigation menu list for the current user.
#[derive(Debug, Serialize, ToSchema)]
pub struct MenusResponse {
    /// Monotonically increasing version counter.
    /// Clients should combine with the `ETag` response header for cache validation.
    pub version: i64,
    /// Only menus the current user has permission to see, ordered by `section` then `priority`.
    pub menus: Vec<MenuResponse>,
}
