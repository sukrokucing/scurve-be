use async_trait::async_trait;
use serde_json::Value;

use super::principal::{Principal, ResourceContext};

/// Policy evaluator trait for pluggable authorization logic
#[async_trait]
pub trait PolicyEvaluator: Send + Sync {
    /// Check if the principal has permission to perform an action
    async fn can(&self, principal: &Principal, permission: &str, ctx: &ResourceContext) -> bool;
}

/// Default policy evaluator with standard RBAC logic
///
/// Evaluation order:
/// 1. super_admin role -> allow
/// 2. direct user permissions (global + scope match) -> allow
/// 3. role permissions (global) -> allow
/// 4. deny
#[derive(Debug, Clone, Default)]
pub struct DefaultPolicyEvaluator;

impl DefaultPolicyEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Check if a scope matches the resource context
    fn scope_matches(scope: &Value, ctx: &ResourceContext) -> bool {
        // Empty scope matches everything
        if scope.is_null() || scope.as_object().map(|o| o.is_empty()).unwrap_or(false) {
            return true;
        }

        let scope_obj = match scope.as_object() {
            Some(o) => o,
            None => return false,
        };

        // Check project_id scope
        if let Some(scope_project) = scope_obj.get("project_id").and_then(|v| v.as_str()) {
            if let Some(ctx_project) = &ctx.project_id {
                if scope_project != ctx_project.to_string() {
                    return false;
                }
            } else {
                // Scope specifies project but context doesn't have one
                return false;
            }
        }

        // Check resource_type and resource_id
        if let Some(scope_rt) = scope_obj.get("resource_type").and_then(|v| v.as_str()) {
            if let Some(ctx_rt) = &ctx.resource_type {
                if scope_rt != ctx_rt {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(scope_rid) = scope_obj.get("resource_id").and_then(|v| v.as_str()) {
            if let Some(ctx_rid) = &ctx.resource_id {
                if scope_rid != ctx_rid.to_string() {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

#[async_trait]
impl PolicyEvaluator for DefaultPolicyEvaluator {
    async fn can(&self, principal: &Principal, permission: &str, ctx: &ResourceContext) -> bool {
        // 1. Super admin bypasses all checks
        if principal.is_super_admin() {
            tracing::debug!(
                user_id = %principal.user_id,
                permission = %permission,
                "super_admin bypass"
            );
            return true;
        }

        // 2. Check direct user permissions (global)
        if principal.has_permission(permission) {
            tracing::debug!(
                user_id = %principal.user_id,
                permission = %permission,
                "direct permission match"
            );
            return true;
        }

        // 3. Check scoped permissions
        for (perm_name, scope) in &principal.scoped_permissions {
            if perm_name == permission && Self::scope_matches(scope, ctx) {
                tracing::debug!(
                    user_id = %principal.user_id,
                    permission = %permission,
                    scope = ?scope,
                    "scoped permission match"
                );
                return true;
            }
        }

        // 4. Deny
        tracing::debug!(
            user_id = %principal.user_id,
            permission = %permission,
            "permission denied"
        );
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_super_admin_bypasses_all() {
        let evaluator = DefaultPolicyEvaluator::new();
        let principal = Principal::new(Uuid::new_v4())
            .with_roles(vec!["super_admin".to_string()]);
        let ctx = ResourceContext::new();

        assert!(evaluator.can(&principal, "anything.at.all", &ctx).await);
    }

    #[tokio::test]
    async fn test_direct_permission_allows() {
        let evaluator = DefaultPolicyEvaluator::new();
        let principal = Principal::new(Uuid::new_v4())
            .with_permissions(vec!["project.create".to_string()]);
        let ctx = ResourceContext::new();

        assert!(evaluator.can(&principal, "project.create", &ctx).await);
        assert!(!evaluator.can(&principal, "project.delete", &ctx).await);
    }

    #[tokio::test]
    async fn test_scoped_permission_matches() {
        let evaluator = DefaultPolicyEvaluator::new();
        let project_id = Uuid::new_v4();
        let scope = serde_json::json!({"project_id": project_id.to_string()});

        let principal = Principal::new(Uuid::new_v4())
            .with_scoped_permissions(vec![("project.view".to_string(), scope)]);

        let ctx = ResourceContext::new().with_project(project_id);
        assert!(evaluator.can(&principal, "project.view", &ctx).await);

        // Different project should not match
        let other_ctx = ResourceContext::new().with_project(Uuid::new_v4());
        assert!(!evaluator.can(&principal, "project.view", &other_ctx).await);
    }

    #[tokio::test]
    async fn test_empty_scope_matches_all() {
        let evaluator = DefaultPolicyEvaluator::new();
        let scope = serde_json::json!({});

        let principal = Principal::new(Uuid::new_v4())
            .with_scoped_permissions(vec![("project.view".to_string(), scope)]);

        let ctx = ResourceContext::new().with_project(Uuid::new_v4());
        assert!(evaluator.can(&principal, "project.view", &ctx).await);
    }

    #[tokio::test]
    async fn test_denial_when_no_permission() {
        let evaluator = DefaultPolicyEvaluator::new();
        let principal = Principal::new(Uuid::new_v4());
        let ctx = ResourceContext::new();

        assert!(!evaluator.can(&principal, "project.create", &ctx).await);
    }
}
