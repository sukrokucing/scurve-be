//! Route Permission Cache
//!
//! Caches route-to-permission mappings from the database for fast lookup.
//! Supports pattern matching for dynamic route segments like `:id`.

use regex::Regex;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cached route-permission mapping
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RouteMapping {
    pub pattern: String,
    pub method: String,
    pub permission: String,
    pub regex: Regex,
}

/// Cache for route-to-permission mappings
#[derive(Debug, Clone)]
pub struct RoutePermissionCache {
    mappings: Arc<RwLock<Vec<RouteMapping>>>,
}

impl RoutePermissionCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            mappings: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Load mappings from the database
    pub async fn load(pool: &SqlitePool) -> Result<Self, sqlx::Error> {
        let cache = Self::new();
        cache.refresh(pool).await?;
        Ok(cache)
    }

    /// Refresh mappings from the database
    pub async fn refresh(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT route_pattern, method, permission_name FROM route_permissions",
        )
        .fetch_all(pool)
        .await?;

        let mappings: Vec<RouteMapping> = rows
            .into_iter()
            .filter_map(|(pattern, method, permission)| {
                let regex = pattern_to_regex(&pattern).ok()?;
                Some(RouteMapping {
                    pattern,
                    method,
                    permission,
                    regex,
                })
            })
            .collect();

        let mut cache = self.mappings.write().await;
        *cache = mappings;

        tracing::info!(
            count = cache.len(),
            "Loaded route permission mappings from database"
        );

        Ok(())
    }

    /// Get the required permission for a given method and path
    pub async fn get_permission(&self, method: &str, path: &str) -> Option<String> {
        let mappings = self.mappings.read().await;

        // Find the first matching pattern
        for mapping in mappings.iter() {
            if mapping.method.eq_ignore_ascii_case(method) && mapping.regex.is_match(path) {
                return Some(mapping.permission.clone());
            }
        }

        None
    }

    /// Check if the cache is empty
    #[allow(dead_code)]
    pub async fn is_empty(&self) -> bool {
        self.mappings.read().await.is_empty()
    }

    /// Get count of cached mappings
    #[allow(dead_code)]
    pub async fn len(&self) -> usize {
        self.mappings.read().await.len()
    }

    /// Validate all routes in the OpenAPI spec against the cache
    pub async fn validate_against_openapi(&self, openapi: &utoipa::openapi::OpenApi) {
        for (path, item) in openapi.paths.paths.iter() {
            // Replace {param} with a dummy value that matches our regex [^/]+
            // e.g. /projects/{id} -> /projects/val
            let test_path = path.replace(['{', '}'], "");

            for (method_type, _operation) in item.operations.iter() {
                let method_str = match method_type {
                    utoipa::openapi::path::PathItemType::Get => "GET",
                    utoipa::openapi::path::PathItemType::Post => "POST",
                    utoipa::openapi::path::PathItemType::Put => "PUT",
                    utoipa::openapi::path::PathItemType::Delete => "DELETE",
                    utoipa::openapi::path::PathItemType::Options => "OPTIONS",
                    utoipa::openapi::path::PathItemType::Head => "HEAD",
                    utoipa::openapi::path::PathItemType::Patch => "PATCH",
                    _ => continue,
                };

                if self.get_permission(method_str, &test_path).await.is_none() {
                    // Skip public routes (health and auth)
                    if path.starts_with("/api/health") || path.starts_with("/auth") {
                        continue;
                    }

                    tracing::warn!(
                        "Route {} {} found in OpenAPI but missing from dynamic permission cache. It will be unreachable in strict mode.",
                        method_str,
                        path
                    );
                }
            }
        }
    }
}

impl Default for RoutePermissionCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a route pattern like `/projects/:id` to a regex like `^/projects/[^/]+$`
fn pattern_to_regex(pattern: &str) -> Result<Regex, regex::Error> {
    if pattern == "/" {
        return Regex::new(r"^/$");
    }

    let mut regex = String::from("^");
    for segment in pattern.split('/') {
        if segment.is_empty() {
            continue;
        }
        regex.push('/');
        if segment.starts_with(':') {
            regex.push_str("[^/]+");
        } else {
            regex.push_str(&regex::escape(segment));
        }
    }
    regex.push('$');

    Regex::new(&regex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_to_regex() {
        let regex = pattern_to_regex("/projects/:id").unwrap();
        assert!(regex.is_match("/projects/abc-123"));
        assert!(regex.is_match("/projects/123"));
        assert!(!regex.is_match("/projects/"));
        assert!(!regex.is_match("/projects/abc/extra"));
    }

    #[test]
    fn test_nested_params() {
        let regex = pattern_to_regex("/projects/:project_id/tasks/:id").unwrap();
        assert!(regex.is_match("/projects/abc/tasks/def"));
        assert!(!regex.is_match("/projects/abc/tasks"));
    }

    #[test]
    fn test_arbitrary_param_names() {
        let regex = pattern_to_regex("/projects/:project_id/resource-roles/:resource_role_id/rate")
            .unwrap();
        assert!(regex.is_match("/projects/abc/resource-roles/def/rate"));
        assert!(!regex.is_match("/projects/abc/resource-roles/def"));
    }

    #[tokio::test]
    async fn test_cache_get_permission() {
        let cache = RoutePermissionCache::new();
        {
            let mut mappings = cache.mappings.write().await;
            mappings.push(RouteMapping {
                pattern: "/projects".to_string(),
                method: "GET".to_string(),
                permission: "project.view".to_string(),
                regex: pattern_to_regex("/projects").unwrap(),
            });
            mappings.push(RouteMapping {
                pattern: "/projects/:id".to_string(),
                method: "DELETE".to_string(),
                permission: "project.delete".to_string(),
                regex: pattern_to_regex("/projects/:id").unwrap(),
            });
        }

        assert_eq!(
            cache.get_permission("GET", "/projects").await,
            Some("project.view".to_string())
        );
        assert_eq!(
            cache.get_permission("DELETE", "/projects/123").await,
            Some("project.delete".to_string())
        );
        assert_eq!(cache.get_permission("POST", "/projects").await, None);
    }
}
