//! `ServiceType` — the module family a managed service belongs to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceType {
    Api,
    Agent,
    Mcp,
}

impl fmt::Display for ServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api => write!(f, "api"),
            Self::Agent => write!(f, "agent"),
            Self::Mcp => write!(f, "mcp"),
        }
    }
}

impl ServiceType {
    pub fn from_module_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "agent" => Self::Agent,
            "api" => Self::Api,
            _ => Self::Mcp,
        }
    }
}
