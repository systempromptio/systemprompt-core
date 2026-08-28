//! Service and database status section payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusSectionData {
    pub services: Vec<ServiceStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<DatabaseStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_errors: Option<ErrorCounts>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceStatus {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseStatus {
    pub size_mb: f64,
    pub status: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct ErrorCounts {
    pub critical: i32,
    pub error: i32,
    pub warn: i32,
}

impl StatusSectionData {
    pub const fn new(services: Vec<ServiceStatus>) -> Self {
        Self {
            services,
            database: None,
            recent_errors: None,
        }
    }

    pub fn with_database(mut self, status: DatabaseStatus) -> Self {
        self.database = Some(status);
        self
    }

    pub const fn with_error_counts(mut self, counts: ErrorCounts) -> Self {
        self.recent_errors = Some(counts);
        self
    }
}

impl ServiceStatus {
    pub fn new(name: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: status.into(),
            uptime: None,
        }
    }

    pub fn with_uptime(mut self, uptime: impl Into<String>) -> Self {
        self.uptime = Some(uptime.into());
        self
    }
}

impl DatabaseStatus {
    pub fn new(size_mb: f64, status: impl Into<String>) -> Self {
        Self {
            size_mb,
            status: status.into(),
        }
    }
}

impl ErrorCounts {
    pub const fn new(critical: i32, error: i32, warn: i32) -> Self {
        Self {
            critical,
            error,
            warn,
        }
    }
}
