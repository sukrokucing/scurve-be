use axum::{body::Body, http::Request, middleware::Next, response::Response};

use super::{AuthzMode, DefaultPolicyEvaluator, PolicyEvaluator, Principal, ResourceContext};
use crate::app::AppState;
use crate::errors::AppError;
use crate::jwt::AuthUser;

use axum::extract::State;

/// Dynamic authorization middleware that checks route_permissions table
///
/// This middleware:
/// 1. Extracts the current request path and HTTP method
/// 2. Looks up the required permission from the route_permissions cache
/// 3. Checks if the authenticated user has that permission
/// 4. Returns 403 if denied (in strict mode) or logs and allows (in advisory mode)
pub async fn dynamic_authz(
    State(state): State<AppState>,
    auth: AuthUser,
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let mode = AuthzMode::from_env();
    if mode == AuthzMode::Off {
        return Ok(next.run(req).await);
    }

    let method = req.method().as_str();
    let path = req.uri().path();

    // Look up required permission from cache
    let permission = state
        .route_permission_cache
        .get_permission(method, path)
        .await;

    match permission {
        Some(perm) => {
            // Load principal and check permission
            let principal = Principal::load(auth.user_id, &state.pool)
                .await
                .map_err(|e| AppError::internal(format!("Failed to load principal: {}", e)))?;

            // Build resource context from path (extract project_id if present)
            let ctx = extract_resource_context(method, path);

            let evaluator = DefaultPolicyEvaluator::new();
            let allowed = evaluator.can(&principal, &perm, &ctx).await;

            if allowed {
                Ok(next.run(req).await)
            } else {
                tracing::warn!(
                    user_id = %auth.user_id,
                    permission = %perm,
                    path = %path,
                    method = %method,
                    mode = ?mode,
                    "Permission denied"
                );

                if mode == AuthzMode::Strict {
                    Err(AppError::forbidden("Permission denied"))
                } else {
                    // Advisory mode: log but allow
                    Ok(next.run(req).await)
                }
            }
        }
        None => {
            // Route not in route_permissions table
            tracing::warn!(
                path = %path,
                method = %method,
                "Route not configured in route_permissions table"
            );

            if mode == AuthzMode::Strict {
                // Deny by default for unconfigured routes
                Err(AppError::forbidden("Route not configured"))
            } else {
                // Advisory/Off mode: allow
                Ok(next.run(req).await)
            }
        }
    }
}

/// Extract resource context from the request path
fn extract_resource_context(method: &str, path: &str) -> ResourceContext {
    let mut ctx = ResourceContext::new();

    // Extract project_id from paths like /projects/{uuid}/...
    let parts: Vec<&str> = path.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "projects" {
            if let Some(id) = parts.get(i + 1) {
                if let Ok(_uuid) = uuid::Uuid::parse_str(id) {
                    ctx = ctx.with_project(*id);
                }
            }
        }
    }

    // Some list-style endpoints are project-agnostic in the URL but should still
    // be reachable with scoped project permissions.
    let is_project_agnostic_list = (method.eq_ignore_ascii_case("GET") && path == "/projects")
        || (method.eq_ignore_ascii_case("GET") && path == "/users/me/projects")
        || (method.eq_ignore_ascii_case("GET") && path == "/portfolio/s-curve/summary");

    if is_project_agnostic_list {
        ctx = ctx.allow_project_scoped_without_target();
    }

    ctx
}
