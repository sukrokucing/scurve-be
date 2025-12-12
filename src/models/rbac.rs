use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::events::{Loggable, Severity};

// =============================================================================
// ROLE
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Loggable for Role {
    fn entity_type() -> &'static str { "role" }
    fn subject_id(&self) -> Uuid { self.id }
    fn severity(&self) -> Severity { Severity::Critical }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbRole {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbRole> for Role {
    fn from(db: DbRole) -> Self {
        Role {
            id: db.id,
            name: db.name,
            description: db.description,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RoleCreateRequest {
    #[schema(example = "project_admin")]
    pub name: String,
    #[schema(example = "Can manage all aspects of projects")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct RoleUpdateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

// =============================================================================
// PERMISSION
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Permission {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Loggable for Permission {
    fn entity_type() -> &'static str { "permission" }
    fn subject_id(&self) -> Uuid { self.id }
    fn severity(&self) -> Severity { Severity::Critical }
}

#[derive(Debug, Clone, FromRow)]
pub struct DbPermission {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbPermission> for Permission {
    fn from(db: DbPermission) -> Self {
        Permission {
            id: db.id,
            name: db.name,
            description: db.description,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PermissionCreateRequest {
    #[schema(example = "project.archive")]
    pub name: String,
    #[schema(example = "Archive completed projects")]
    pub description: Option<String>,
}

// =============================================================================
// USER-ROLE ASSIGNMENT
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserRole {
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Loggable for UserRole {
    fn entity_type() -> &'static str { "user_role" }
    fn subject_id(&self) -> Uuid { self.user_id }
    fn severity(&self) -> Severity { Severity::Critical }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignRoleRequest {
    pub role_id: Uuid,
}

// =============================================================================
// USER-PERMISSION DIRECT GRANT
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserPermission {
    pub id: Uuid,
    pub user_id: Uuid,
    pub permission_id: Uuid,
    /// Scope as JSON for resource-level permissions
    #[serde(default)]
    #[schema(value_type = Object)]
    pub scope: Value,
    pub created_at: DateTime<Utc>,
}

impl Loggable for UserPermission {
    fn entity_type() -> &'static str { "user_permission" }
    fn subject_id(&self) -> Uuid { self.user_id }
    fn severity(&self) -> Severity { Severity::Critical }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GrantPermissionRequest {
    pub permission_id: Uuid,
    /// Optional scope JSON, e.g. {"project_id": "..."}
    #[serde(default)]
    #[schema(value_type = Object)]
    pub scope: Option<Value>,
}

// =============================================================================
// ROLE-PERMISSION ASSIGNMENT
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RolePermission {
    pub role_id: Uuid,
    pub permission_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Loggable for RolePermission {
    fn entity_type() -> &'static str { "role_permission" }
    fn subject_id(&self) -> Uuid { self.role_id }
    fn severity(&self) -> Severity { Severity::Critical }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignPermissionToRoleRequest {
    pub permission_id: Uuid,
}

// =============================================================================
// EFFECTIVE PERMISSIONS (computed)
// =============================================================================

#[derive(Debug, Serialize, ToSchema)]
pub struct EffectivePermissions {
    pub user_id: Uuid,
    pub roles: Vec<String>,
    pub permissions: Vec<EffectivePermission>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EffectivePermission {
    pub name: String,
    /// Source of the permission: "role" or "direct"
    #[schema(example = "role")]
    pub source: String,
    /// Name of the role if source is "role"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object)]
    pub scope: Option<Value>,
}
