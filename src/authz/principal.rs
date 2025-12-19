use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

/// Principal represents the authenticated user with their cached permissions
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub roles: HashSet<String>,
    pub permissions: HashSet<String>,
    /// Scoped permissions: (permission_name, scope_json)
    pub scoped_permissions: Vec<(String, Value)>,
}

use sqlx::{Row, SqlitePool};

impl Principal {
    #[allow(dead_code)]
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            roles: HashSet::new(),
            permissions: HashSet::new(),
            scoped_permissions: Vec::new(),
        }
    }

    /// Load roles and permissions from the database for a given user
    pub async fn load(user_id: Uuid, pool: &SqlitePool) -> Result<Self, sqlx::Error> {
        use crate::db::uuid_sql::match_uuid_clause;

        // 1. Load role names
        let role_match = match_uuid_clause("ur.user_id");
        let role_sql = format!(
            "SELECT r.name FROM roles r INNER JOIN user_roles ur ON r.id = ur.role_id WHERE {}",
            role_match
        );
        let role_rows = sqlx::query(&role_sql)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(pool)
            .await?;
        let roles: HashSet<String> = role_rows.iter().map(|r| r.get("name")).collect();

        // 2. Load global permissions from roles
        let perm_match = match_uuid_clause("ur.user_id");
        let perm_sql = format!(
            r#"
            SELECT DISTINCT p.name
            FROM permissions p
            INNER JOIN role_permissions rp ON p.id = rp.permission_id
            INNER JOIN user_roles ur ON rp.role_id = ur.role_id
            WHERE {}
            "#,
            perm_match
        );
        let role_perm_rows = sqlx::query(&perm_sql)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(pool)
            .await?;
        let mut permissions: HashSet<String> = role_perm_rows.iter().map(|r| r.get("name")).collect();

        // 3. Load direct permissions and scoped permissions
        let direct_match = match_uuid_clause("up.user_id");
        let direct_sql = format!(
            r#"
            SELECT p.name, up.scope
            FROM permissions p
            INNER JOIN user_permissions up ON p.id = up.permission_id
            WHERE {}
            "#,
            direct_match
        );
        let direct_perm_rows = sqlx::query(&direct_sql)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(pool)
            .await?;

        let mut scoped_permissions = Vec::new();
        for r in direct_perm_rows {
            let name: String = r.get("name");
            let scope_str: Option<String> = r.get("scope");
            let scope: Value = scope_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);

            if scope.is_null() || (scope.is_object() && scope.as_object().unwrap().is_empty()) {
                permissions.insert(name);
            } else {
                scoped_permissions.push((name, scope));
            }
        }

        Ok(Self {
            user_id,
            roles,
            permissions,
            scoped_permissions,
        })
    }

    #[allow(dead_code)]
    pub fn with_roles(mut self, roles: impl IntoIterator<Item = String>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }

    #[allow(dead_code)]
    pub fn with_permissions(mut self, perms: impl IntoIterator<Item = String>) -> Self {
        self.permissions = perms.into_iter().collect();
        self
    }

    #[allow(dead_code)]
    pub fn with_scoped_permissions(mut self, scoped: Vec<(String, Value)>) -> Self {
        self.scoped_permissions = scoped;
        self
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(role)
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    pub fn is_super_admin(&self) -> bool {
        self.has_role(super::roles::SUPER_ADMIN)
    }
}

/// Resource context for scoped permission checks
#[derive(Debug, Clone, Default)]
pub struct ResourceContext {
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    #[allow(dead_code)]
    pub metadata: Option<Value>,
}

impl ResourceContext {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn with_project(mut self, project_id: impl ToString) -> Self {
        self.project_id = uuid::Uuid::parse_str(&project_id.to_string()).ok();
        self
    }

    #[allow(dead_code)]
    pub fn with_resource(mut self, resource_type: &str, resource_id: Uuid) -> Self {
        self.resource_type = Some(resource_type.to_string());
        self.resource_id = Some(resource_id);
        self
    }

    /// Convert to scope JSON for matching against scoped permissions
    #[allow(dead_code)]
    pub fn to_scope_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        if let Some(ref rt) = self.resource_type {
            map.insert("resource_type".to_string(), Value::String(rt.clone()));
        }
        if let Some(ref rid) = self.resource_id {
            map.insert("resource_id".to_string(), Value::String(rid.to_string()));
        }
        if let Some(ref pid) = self.project_id {
            map.insert("project_id".to_string(), Value::String(pid.to_string()));
        }
        Value::Object(map)
    }
}
