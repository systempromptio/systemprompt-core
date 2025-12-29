use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeStatus {
    Running,
    Starting,
    Stopped,
    Crashed,
    Orphaned,
}

impl RuntimeStatus {
    pub const fn is_healthy(&self) -> bool {
        matches!(self, Self::Running | Self::Starting)
    }

    pub const fn needs_cleanup(&self) -> bool {
        matches!(self, Self::Crashed | Self::Orphaned)
    }
}

impl fmt::Display for RuntimeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Starting => write!(f, "starting"),
            Self::Stopped => write!(f, "stopped"),
            Self::Crashed => write!(f, "crashed"),
            Self::Orphaned => write!(f, "orphaned"),
        }
    }
}

impl std::str::FromStr for RuntimeStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "running" => Ok(Self::Running),
            "starting" => Ok(Self::Starting),
            "stopped" => Ok(Self::Stopped),
            "crashed" | "error" => Ok(Self::Crashed),
            "orphaned" => Ok(Self::Orphaned),
            _ => Err(format!("Invalid runtime status: {s}")),
        }
    }
}

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
