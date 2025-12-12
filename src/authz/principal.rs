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

impl Principal {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            roles: HashSet::new(),
            permissions: HashSet::new(),
            scoped_permissions: Vec::new(),
        }
    }

    pub fn with_roles(mut self, roles: impl IntoIterator<Item = String>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }

    pub fn with_permissions(mut self, perms: impl IntoIterator<Item = String>) -> Self {
        self.permissions = perms.into_iter().collect();
        self
    }

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
    pub metadata: Option<Value>,
}

impl ResourceContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_project(mut self, project_id: Uuid) -> Self {
        self.project_id = Some(project_id);
        self
    }

    pub fn with_resource(mut self, resource_type: &str, resource_id: Uuid) -> Self {
        self.resource_type = Some(resource_type.to_string());
        self.resource_id = Some(resource_id);
        self
    }

    /// Convert to scope JSON for matching against scoped permissions
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
