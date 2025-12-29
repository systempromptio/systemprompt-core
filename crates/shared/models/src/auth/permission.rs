use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Admin,
    User,
    Anonymous,
    A2a,
    Mcp,
    Service,
}

impl Permission {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::Anonymous => "anonymous",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Service => "service",
        }
    }

    pub const fn is_user_role(&self) -> bool {
        matches!(self, Self::Admin | Self::User | Self::Anonymous)
    }

    pub const fn is_service_scope(&self) -> bool {
        matches!(self, Self::A2a | Self::Mcp | Self::Service)
    }

    pub const fn hierarchy_level(&self) -> u8 {
        match self {
            Self::Admin => 100,
            Self::User => 50,
            Self::Service => 40,
            Self::A2a => 30,
            Self::Mcp => 20,
            Self::Anonymous => 10,
        }
    }

    pub const fn implies(&self, other: &Self) -> bool {
        self.hierarchy_level() >= other.hierarchy_level()
    }

    pub fn user_permissions() -> Vec<Self> {
        vec![Self::Admin, Self::User, Self::Anonymous]
    }

    pub fn service_permissions() -> Vec<Self> {
        vec![Self::A2a, Self::Mcp, Self::Service]
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Permission {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "anonymous" => Ok(Self::Anonymous),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "service" => Ok(Self::Service),
            _ => Err(anyhow!("Invalid permission: {s}")),
        }
    }
}

pub fn permissions_to_string(permissions: &[Permission]) -> String {
    permissions
        .iter()
        .map(Permission::as_str)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_permissions(s: &str) -> Result<Vec<Permission>> {
    s.split_whitespace().map(Permission::from_str).collect()
}
