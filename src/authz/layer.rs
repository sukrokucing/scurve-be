use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};

use crate::app::AppState;
use crate::errors::AppError;
use crate::jwt::AuthUser;
use uuid::Uuid;
use super::{Principal, ResourceContext, DefaultPolicyEvaluator, PolicyEvaluator, AuthzMode};

use axum::extract::State;

/// Middleware to enforce a specific permission
pub async fn require_permission(
    permission: &'static str,
    state: AppState,
    auth_user: AuthUser,
    params: std::collections::HashMap<String, String>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let mode = AuthzMode::from_env();
    if mode == AuthzMode::Off {
        return Ok(next.run(req).await);
    }

    // Load principal
    let principal = Principal::load(auth_user.user_id, &state.pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to load principal: {}", e)))?;

    // Build resource context from path params
    let mut ctx = ResourceContext::new();

    if let Some(pid) = params.get("project_id") {
        ctx = ctx.with_project(pid);
    }

    if let Some(rid) = params.get("id") {
        if let Ok(uuid) = uuid::Uuid::parse_str(rid) {
            ctx = ctx.with_resource("any", uuid); // Guessing type
        }
    }

    let evaluator = DefaultPolicyEvaluator::new();
    let allowed = evaluator.can(&principal, permission, &ctx).await;

    if allowed {
        Ok(next.run(req).await)
    } else {
        tracing::warn!(
            user_id = %auth_user.user_id,
            permission = %permission,
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

// Named middleware functions for use with from_fn_with_state
// This avoids type inference issues with async closures

macro_rules! define_require_middleware {
    ($name:ident, $perm:expr) => {
        pub async fn $name(
            State(state): State<AppState>,
            auth: AuthUser,
            path: Option<axum::extract::Path<std::collections::HashMap<String, String>>>,
            req: Request<Body>,
            next: Next,
        ) -> Result<Response, AppError> {
            let params = path.map(|p| p.0).unwrap_or_default();
            require_permission($perm, state, auth, params, req, next).await
        }
    };
}

use super::permissions;

define_require_middleware!(require_project_view, permissions::PROJECT_VIEW);
define_require_middleware!(require_project_create, permissions::PROJECT_CREATE);
define_require_middleware!(require_project_update, permissions::PROJECT_UPDATE);
define_require_middleware!(require_project_delete, permissions::PROJECT_DELETE);

define_require_middleware!(require_task_view, permissions::TASK_VIEW);
define_require_middleware!(require_task_create, permissions::TASK_CREATE);
define_require_middleware!(require_task_update, permissions::TASK_UPDATE);
define_require_middleware!(require_task_delete, permissions::TASK_DELETE);

define_require_middleware!(require_progress_view, permissions::PROGRESS_VIEW);
define_require_middleware!(require_progress_create, permissions::PROGRESS_CREATE);

define_require_middleware!(require_role_view, permissions::ROLE_VIEW);
define_require_middleware!(require_role_manage, permissions::ROLE_MANAGE);
define_require_middleware!(require_permission_view, permissions::PERMISSION_VIEW);
define_require_middleware!(require_permission_manage, permissions::PERMISSION_MANAGE);
define_require_middleware!(require_user_view, permissions::USER_VIEW);
define_require_middleware!(require_user_manage, permissions::USER_MANAGE);

/// Helper for handlers to check permissions manually with full context
#[allow(dead_code)]
pub async fn check_permission(
    user_id: Uuid,
    permission: &str,
    ctx: &ResourceContext,
    state: &AppState,
) -> Result<bool, AppError> {
    let mode = AuthzMode::from_env();
    if mode == AuthzMode::Off {
        return Ok(true);
    }

    let principal = Principal::load(user_id, &state.pool)
        .await
        .map_err(|e| AppError::internal(format!("Failed to load principal: {}", e)))?;

    let evaluator = DefaultPolicyEvaluator::new();
    let allowed = evaluator.can(&principal, permission, ctx).await;

    if !allowed {
        tracing::warn!(
            user_id = %user_id,
            permission = %permission,
            mode = ?mode,
            "Permission denied"
        );

        if mode == AuthzMode::Strict {
            return Ok(false);
        }
    }

    Ok(true)
}
