use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};
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

use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

fn decode_uuidish_text_column(row: &SqliteRow, column: &str) -> Result<String, sqlx::Error> {
    if let Ok(value) = row.try_get::<String, _>(column) {
        return Ok(value);
    }

    if let Ok(bytes) = row.try_get::<Vec<u8>, _>(column) {
        if bytes.len() == 16 {
            if let Ok(uuid) = Uuid::from_slice(&bytes) {
                return Ok(uuid.to_string());
            }
        }

        if let Ok(text) = String::from_utf8(bytes.clone()) {
            return Ok(text);
        }

        return Err(sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("column `{}` contains non-utf8 bytes", column),
        ))));
    }

    if let Ok(value) = row.try_get::<i64, _>(column) {
        return Ok(value.to_string());
    }

    if let Ok(value) = row.try_get::<f64, _>(column) {
        return Ok(value.to_string());
    }

    row.try_get::<String, _>(column)
}

#[derive(Clone)]
struct PrincipalCacheEntry {
    principal: Principal,
    expires_at: Instant,
}

fn principal_cache() -> &'static RwLock<HashMap<Uuid, PrincipalCacheEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<Uuid, PrincipalCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn principal_cache_ttl() -> Duration {
    static TTL: OnceLock<Duration> = OnceLock::new();
    *TTL.get_or_init(|| {
        // Default 30 000 ms (30 s). Set AUTHZ_PRINCIPAL_CACHE_MS=0 to disable caching.
        let ms = std::env::var("AUTHZ_PRINCIPAL_CACHE_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30_000);
        Duration::from_millis(ms)
    })
}

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
        let ttl = principal_cache_ttl();
        if ttl.is_zero() {
            return Self::load_uncached(user_id, pool).await;
        }

        let now = Instant::now();
        if let Ok(cache) = principal_cache().read() {
            if let Some(entry) = cache.get(&user_id) {
                if entry.expires_at > now {
                    return Ok(entry.principal.clone());
                }
            }
        }

        let principal = Self::load_uncached(user_id, pool).await?;

        if let Ok(mut cache) = principal_cache().write() {
            let now = Instant::now();
            cache.insert(
                user_id,
                PrincipalCacheEntry {
                    principal: principal.clone(),
                    expires_at: now + ttl,
                },
            );
            if cache.len() > 4096 {
                cache.retain(|_, entry| entry.expires_at > now);
            }
        }

        Ok(principal)
    }

    async fn load_uncached(user_id: Uuid, pool: &SqlitePool) -> Result<Self, sqlx::Error> {
        use crate::db::uuid_sql::{case_uuid, match_uuid_clause};

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
        let mut permissions: HashSet<String> =
            role_perm_rows.iter().map(|r| r.get("name")).collect();

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

        // 4. Load project-scoped permissions from project member roles.
        let member_user_match = match_uuid_clause("pm.user_id");
        let member_project_case = case_uuid("pm.project_id");
        let member_sql = format!(
            r#"
            SELECT
                p.name,
                {}
            FROM project_members pm
            INNER JOIN projects pr ON pr.id = pm.project_id
            INNER JOIN role_permissions rp ON rp.role_id = pm.access_role_id
            INNER JOIN permissions p ON p.id = rp.permission_id
            WHERE {} AND pm.deleted_at IS NULL AND pr.deleted_at IS NULL
            "#,
            member_project_case, member_user_match
        );

        let member_perm_rows = sqlx::query(&member_sql)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(pool)
            .await?;

        let mut seen_scoped: HashSet<(String, String)> = scoped_permissions
            .iter()
            .filter_map(|(name, scope)| {
                scope
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .map(|project_id| (name.clone(), project_id.to_string()))
            })
            .collect();

        for row in member_perm_rows {
            let name: String = row.try_get("name")?;
            let project_id = decode_uuidish_text_column(&row, "project_id")?;
            let key = (name.clone(), project_id.clone());
            if seen_scoped.insert(key) {
                scoped_permissions.push((
                    name,
                    serde_json::json!({
                        "project_id": project_id,
                    }),
                ));
            }
        }

        Ok(Self {
            user_id,
            roles,
            permissions,
            scoped_permissions,
        })
    }

    /// Load a synthetic Principal for "view-as" mode.
    ///
    /// The returned principal carries the real admin's `user_id` but has only the
    /// roles and global permissions of the named role. Scoped (project-level)
    /// permissions are not simulated — view-as is a global role simulation.
    ///
    /// This function never reads from or writes to the principal cache.
    pub async fn load_for_role(
        admin_user_id: Uuid,
        role_name: &str,
        pool: &SqlitePool,
    ) -> Result<Self, sqlx::Error> {
        // Look up the role by name
        let role_row = sqlx::query("SELECT id FROM roles WHERE name = ?")
            .bind(role_name)
            .fetch_optional(pool)
            .await?;

        let role_row = match role_row {
            Some(r) => r,
            None => {
                // Unknown role — return empty principal so the caller can warn and fall back
                return Ok(Self {
                    user_id: admin_user_id,
                    roles: HashSet::new(),
                    permissions: HashSet::new(),
                    scoped_permissions: Vec::new(),
                });
            }
        };

        let role_id: String = role_row.get("id");

        // Load global permissions for this role
        let perm_rows = sqlx::query(
            r#"
            SELECT p.name
            FROM permissions p
            INNER JOIN role_permissions rp ON p.id = rp.permission_id
            WHERE rp.role_id = ?
            "#,
        )
        .bind(&role_id)
        .fetch_all(pool)
        .await?;

        let permissions: HashSet<String> = perm_rows.iter().map(|r| r.get("name")).collect();

        Ok(Self {
            user_id: admin_user_id,
            roles: std::iter::once(role_name.to_string()).collect(),
            permissions,
            scoped_permissions: Vec::new(),
        })
    }

    #[allow(dead_code)]
    pub fn invalidate_cache_for(user_id: Uuid) {
        if let Ok(mut cache) = principal_cache().write() {
            cache.remove(&user_id);
        }
    }

    #[allow(dead_code)]
    pub fn invalidate_cache_all() {
        if let Ok(mut cache) = principal_cache().write() {
            cache.clear();
        }
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

#[cfg(test)]
mod tests {
    use super::Principal;
    use serde_json::json;
    use sqlx::SqlitePool;
    use uuid::Uuid;

    #[tokio::test]
    async fn load_handles_blob_project_id_for_project_members(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;

        sqlx::query("CREATE TABLE roles (id TEXT PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await?;
        sqlx::query("CREATE TABLE user_roles (user_id BLOB, role_id TEXT NOT NULL)")
            .execute(&pool)
            .await?;
        sqlx::query("CREATE TABLE permissions (id TEXT PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await?;
        sqlx::query(
            "CREATE TABLE role_permissions (role_id TEXT NOT NULL, permission_id TEXT NOT NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE user_permissions (user_id TEXT, permission_id TEXT NOT NULL, scope TEXT)",
        )
        .execute(&pool)
        .await?;
        sqlx::query("CREATE TABLE projects (id BLOB PRIMARY KEY, deleted_at TEXT)")
            .execute(&pool)
            .await?;
        sqlx::query(
            "CREATE TABLE project_members (
                id TEXT PRIMARY KEY,
                project_id BLOB NOT NULL,
                user_id BLOB NOT NULL,
                access_role_id TEXT NOT NULL,
                deleted_at TEXT
            )",
        )
        .execute(&pool)
        .await?;

        let user_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let role_id = Uuid::new_v4();
        let permission_id = Uuid::new_v4();

        sqlx::query("INSERT INTO roles (id, name) VALUES (?, ?)")
            .bind(role_id.to_string())
            .bind("project_member")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO permissions (id, name) VALUES (?, ?)")
            .bind(permission_id.to_string())
            .bind("tasks.read")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES (?, ?)")
            .bind(role_id.to_string())
            .bind(permission_id.to_string())
            .execute(&pool)
            .await?;

        sqlx::query("INSERT INTO projects (id, deleted_at) VALUES (?, NULL)")
            .bind(project_id.as_bytes().to_vec())
            .execute(&pool)
            .await?;

        sqlx::query(
            "INSERT INTO project_members (id, project_id, user_id, access_role_id, deleted_at) VALUES (?, ?, ?, ?, NULL)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(project_id.as_bytes().to_vec())
        .bind(user_id.as_bytes().to_vec())
        .bind(role_id.to_string())
        .execute(&pool)
        .await?;

        let principal = Principal::load(user_id, &pool).await?;
        assert!(principal.scoped_permissions.contains(&(
            "tasks.read".to_string(),
            json!({ "project_id": project_id.to_string() })
        )));

        Ok(())
    }
}

/// Resource context for scoped permission checks
#[derive(Debug, Clone, Default)]
pub struct ResourceContext {
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub allow_project_scoped_without_target: bool,
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

    pub fn allow_project_scoped_without_target(mut self) -> Self {
        self.allow_project_scoped_without_target = true;
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
