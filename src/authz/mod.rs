//! Authorization module - Policy Engine and Middleware
//!
//! This module implements the RBAC policy engine with support for:
//! - Role-based permissions
//! - Direct user permissions (with optional scope)
//! - Super admin bypass
//! - Configurable enforcement modes (off/advisory/strict)

mod evaluator;
mod principal;

pub use evaluator::{DefaultPolicyEvaluator, PolicyEvaluator};
pub use principal::{Principal, ResourceContext};

use std::sync::OnceLock;

/// Authorization enforcement mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthzMode {
    /// No permission checks (development mode)
    Off,
    /// Log denials but allow requests (testing mode)
    Advisory,
    /// Enforce 403 on denied requests (production mode)
    Strict,
}

impl AuthzMode {
    pub fn from_env() -> Self {
        static MODE: OnceLock<AuthzMode> = OnceLock::new();
        *MODE.get_or_init(|| {
            match std::env::var("AUTHZ_MODE").unwrap_or_default().to_lowercase().as_str() {
                "advisory" => AuthzMode::Advisory,
                "strict" => AuthzMode::Strict,
                _ => AuthzMode::Off,
            }
        })
    }
}

/// Well-known role names
pub mod roles {
    pub const SUPER_ADMIN: &str = "super_admin";
    pub const ADMIN: &str = "admin";
    pub const PROJECT_MANAGER: &str = "project_manager";
    pub const MEMBER: &str = "member";
    pub const VIEWER: &str = "viewer";
}

/// Well-known permission names
pub mod permissions {
    // Project
    pub const PROJECT_CREATE: &str = "project.create";
    pub const PROJECT_VIEW: &str = "project.view";
    pub const PROJECT_UPDATE: &str = "project.update";
    pub const PROJECT_DELETE: &str = "project.delete";

    // Task
    pub const TASK_CREATE: &str = "task.create";
    pub const TASK_VIEW: &str = "task.view";
    pub const TASK_UPDATE: &str = "task.update";
    pub const TASK_DELETE: &str = "task.delete";

    // Progress
    pub const PROGRESS_CREATE: &str = "progress.create";
    pub const PROGRESS_VIEW: &str = "progress.view";

    // User
    pub const USER_VIEW: &str = "user.view";
    pub const USER_MANAGE: &str = "user.manage";

    // RBAC
    pub const ROLE_VIEW: &str = "role.view";
    pub const ROLE_MANAGE: &str = "role.manage";
    pub const PERMISSION_VIEW: &str = "permission.view";
    pub const PERMISSION_MANAGE: &str = "permission.manage";
}
