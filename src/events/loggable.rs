use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Severity levels for activity logs (Phase 5).
/// Controls retention policies and log filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Critical events: long-term retention, never auto-delete
    Critical,
    /// Important events: medium-term retention (default)
    Important,
    /// Noise events: aggressively trimmed (e.g., 7 days)
    Noise,
}

impl Severity {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::Important => "important",
            Severity::Noise => "noise",
        }
    }
}

impl Default for Severity {
    fn default() -> Self {
        Severity::Important
    }
}

/// Trait for entities that can be logged in the activity log.
/// Implement this trait on any model to enable declarative activity logging.
pub trait Loggable: Serialize + Send + Sync {
    /// The entity type name (e.g., "task", "project", "user")
    /// This becomes the prefix in event names like "task.created"
    fn entity_type() -> &'static str;

    /// The subject ID (usually the entity's primary key)
    fn subject_id(&self) -> Uuid;

    /// Severity level for logs (defaults to Important)
    fn severity(&self) -> Severity {
        Severity::Important
    }

    /// Override severity based on action (e.g., "deleted" -> Critical)
    fn severity_for_action(&self, action: &str) -> Severity {
        match action {
            "deleted" => Severity::Critical,
            "created" | "updated" => self.severity(),
            _ => Severity::Important,
        }
    }
}
