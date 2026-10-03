//! Closed vocabularies of the platform-wide `services` registry table.
//!
//! [`ServiceModule`] is the `services.module_name` column (which module owns
//! the row) and [`ServiceStatus`] the `services.status` column (the lifecycle
//! state the owning orchestrator last wrote). `as_str` is the stored string
//! and `FromStr` the only way back from it, so a value no orchestrator writes
//! is a decode error rather than a silent fall-through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceModule {
    Mcp,
    Agent,
}

impl ServiceModule {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mcp => "mcp",
            Self::Agent => "agent",
        }
    }
}

impl fmt::Display for ServiceModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown service module `{0}`")]
pub struct UnknownServiceModule(pub String);

impl FromStr for ServiceModule {
    type Err = UnknownServiceModule;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mcp" => Ok(Self::Mcp),
            "agent" => Ok(Self::Agent),
            other => Err(UnknownServiceModule(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl ServiceStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown service status `{0}`")]
pub struct UnknownServiceStatus(pub String);

impl FromStr for ServiceStatus {
    type Err = UnknownServiceStatus;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "stopped" => Ok(Self::Stopped),
            "error" => Ok(Self::Error),
            other => Err(UnknownServiceStatus(other.to_owned())),
        }
    }
}
