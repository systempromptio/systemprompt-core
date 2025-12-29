//! Agent identifier types.

use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Agent identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ToDbValue for AgentId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &AgentId {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

/// Agent identifier for request routing and task attribution
///
/// Represents the name/ID of an agent service that handles requests.
/// Unlike [`ClientId`] (OAuth), this identifies which agent service processes
/// the request, not which application made it.
///
/// # Format
/// - Lowercase alphanumeric with hyphens
/// - Examples: "edward", "content-research", "system"
/// - Cannot be empty or "unknown"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct AgentName(String);

impl AgentName {
    /// Create a new agent name
    ///
    /// # Panics
    /// - If name is empty
    /// - If name is "unknown" (reserved for error detection)
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        assert!(!name.is_empty(), "Agent name cannot be empty");
        assert_ne!(
            name.to_lowercase().as_str(),
            "unknown",
            "Agent name 'unknown' is reserved for error detection"
        );
        Self(name)
    }

    /// Get the agent name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a system agent name
    pub fn system() -> Self {
        Self("system".to_string())
    }
}

impl AsRef<str> for AgentName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for AgentName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for AgentName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl ToDbValue for AgentName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &AgentName {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
