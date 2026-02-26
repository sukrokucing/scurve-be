//! Authorization module - Policy Engine and Middleware
//!
//! This module implements the RBAC policy engine with support for:
//! - Role-based permissions
//! - Direct user permissions (with optional scope)
//! - Super admin bypass
//! - Configurable enforcement modes (off/advisory/strict)
//! - Database-driven route-permission mappings

mod evaluator;
mod principal;
pub mod layer;
pub mod route_cache;

// Re-export common items
pub use evaluator::{DefaultPolicyEvaluator, PolicyEvaluator};
pub use principal::{Principal, ResourceContext};
pub use route_cache::RoutePermissionCache;

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
#[allow(dead_code)]
pub mod roles {
    pub const SUPER_ADMIN: &str = "super_admin";
    pub const ADMIN: &str = "admin";
    pub const PROJECT_MANAGER: &str = "project_manager";
    pub const MEMBER: &str = "member";
    pub const VIEWER: &str = "viewer";
}

/// Well-known permission names (Generated from permissions.json)
#[allow(dead_code)]
pub mod permissions {
    include!(concat!(env!("OUT_DIR"), "/permissions_generated.rs"));
}
