use axum::{body::Body, http::Request, middleware::Next, response::Response};
use uuid::Uuid;

use super::{roles, AuthzMode, DefaultPolicyEvaluator, PolicyEvaluator, Principal, ResourceContext};
use crate::app::AppState;
use crate::errors::AppError;
use crate::jwt::AuthUser;

use axum::extract::State;

/// Inserted into request extensions by [`dynamic_authz`] when an admin sends view-as headers.
///
/// Handlers extract this via `Option<axum::Extension<ViewAsContext>>` to apply data-level
/// filtering for the impersonated identity instead of the real admin's identity.
///
/// - `effective_user_id = Some(uid)` — view-as-user: use `uid` for all membership/data queries
/// - `effective_user_id = None` — view-as-role simulation: no specific user; callers should use
///   `Uuid::nil()` which naturally returns no rows (no project owner or member has that ID)
#[derive(Debug, Clone)]
pub struct ViewAsContext {
    pub effective_user_id: Option<Uuid>,
}

/// Returns the user ID that handlers should use for data-level DB filtering.
///
/// When a view-as extension is present, uses the impersonated user's ID (or `Uuid::nil()` for
/// role simulation, which matches no rows). When absent, returns `auth_user_id` unchanged.
pub fn resolve_data_user(
    view_as: &Option<axum::Extension<ViewAsContext>>,
    auth_user_id: Uuid,
) -> Uuid {
    match view_as {
        None => auth_user_id,
        Some(axum::Extension(ctx)) => ctx.effective_user_id.unwrap_or(Uuid::nil()),
    }
}

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
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let mode = AuthzMode::from_env();
    if mode == AuthzMode::Off {
        return Ok(next.run(req).await);
    }

    // Clone method/path to owned Strings so we can later mutably borrow req for extensions
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();

    // Look up required permission from cache
    let permission = state
        .route_permission_cache
        .get_permission(&method, &path)
        .await;

    match permission {
        Some(perm) => {
            // Load the real principal (DB/cache)
            let real_principal = Principal::load(auth.user_id, &state.pool)
                .await
                .map_err(|e| AppError::internal(format!("Failed to load principal: {}", e)))?;

            // View-as override — admin and super_admin only; non-admins have headers ignored.
            //
            // X-View-As-User takes priority over X-View-As-Role.
            //   X-View-As-User: <uuid>       — load the target user's full principal
            //                                  (roles + permissions + project scopes)
            //   X-View-As-Role: <role-name>  — load a synthetic principal for a named role
            //                                  (global permissions only, no project scopes)
            let view_as_user_id = req
                .headers()
                .get("x-view-as-user")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| uuid::Uuid::parse_str(s.trim()).ok());

            let view_as_role = req
                .headers()
                .get("x-view-as-role")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let is_privileged =
                real_principal.has_role(roles::ADMIN) || real_principal.is_super_admin();

            // Compute effective principal AND view-as context for data-level filtering.
            // view_as_ctx is inserted into request extensions so handlers can use the correct
            // user ID for their DB queries instead of the real admin's user ID.
            let (principal, view_as_ctx) = if let Some(target_user_id) = view_as_user_id {
                if is_privileged {
                    tracing::info!(
                        admin_id = %auth.user_id,
                        view_as_user_id = %target_user_id,
                        path = %path,
                        method = %method,
                        "Admin using view-as-user mode"
                    );
                    let p = Principal::load(target_user_id, &state.pool)
                        .await
                        .map_err(|e| {
                            AppError::internal(format!("view-as-user load failed: {}", e))
                        })?;
                    (p, Some(ViewAsContext { effective_user_id: Some(target_user_id) }))
                } else {
                    (real_principal, None)
                }
            } else if let Some(ref role_name) = view_as_role {
                if is_privileged {
                    let synthetic =
                        Principal::load_for_role(auth.user_id, role_name, &state.pool)
                            .await
                            .map_err(|e| {
                                AppError::internal(format!("view-as-role load failed: {}", e))
                            })?;

                    if synthetic.roles.is_empty() {
                        tracing::warn!(
                            admin_id = %auth.user_id,
                            view_as_role = %role_name,
                            "view-as requested unknown role, ignoring override"
                        );
                        (real_principal, None)
                    } else {
                        tracing::info!(
                            admin_id = %auth.user_id,
                            view_as_role = %role_name,
                            path = %path,
                            method = %method,
                            "Admin using view-as-role mode"
                        );
                        // Role simulation: no specific user identity → data_user_id returns nil UUID
                        (synthetic, Some(ViewAsContext { effective_user_id: None }))
                    }
                } else {
                    (real_principal, None)
                }
            } else {
                (real_principal, None)
            };

            // Propagate view-as context to handlers for data-level filtering
            if let Some(ctx) = view_as_ctx {
                req.extensions_mut().insert(ctx);
            }

            // Build resource context from path (extract project_id if present)
            let ctx = extract_resource_context(&method, &path);

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
                    view_as_user = ?view_as_user_id,
                    view_as_role = ?view_as_role,
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
        || (method.eq_ignore_ascii_case("GET") && path == "/portfolio/s-curve/summary")
        || path == "/notifications"
        || path == "/notifications/unread-count"
        || path == "/notifications/read"
        || path == "/notifications/read-all"
        || path == "/realtime/ws";

    if is_project_agnostic_list {
        ctx = ctx.allow_project_scoped_without_target();
    }

    ctx
}
