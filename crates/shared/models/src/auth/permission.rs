//! The [`Permission`] scope set and its hierarchy.
//!
//! [`Permission`] is the PBAC grant carried in a token's `scope`. The
//! privilege ordering ([`Permission::hierarchy_level`],
//! [`Permission::implies`]) drives every route-level access check, and
//! [`parse_permissions`] / [`permissions_to_string`] are the space-delimited
//! wire codec.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::enums::UserType;
use crate::errors::ParseEnumError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Admin,
    User,
    Anonymous,
    A2a,
    Mcp,
    Service,
    HookGovern,
    HookTrack,
}

impl Permission {
    pub const ALL_VARIANTS: &'static [&'static str] = &[
        "admin",
        "user",
        "anonymous",
        "a2a",
        "mcp",
        "service",
        "hook:govern",
        "hook:track",
    ];

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::Anonymous => "anonymous",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Service => "service",
            Self::HookGovern => "hook:govern",
            Self::HookTrack => "hook:track",
        }
    }

    pub fn is_valid_role(role: &str) -> bool {
        Self::ALL_VARIANTS.contains(&role)
    }

    pub fn validate_roles(roles: &[String]) -> Result<(), Vec<String>> {
        let invalid: Vec<String> = roles
            .iter()
            .filter(|r| !Self::is_valid_role(r))
            .cloned()
            .collect();

        if invalid.is_empty() {
            Ok(())
        } else {
            Err(invalid)
        }
    }

    pub const fn as_user_type(self) -> UserType {
        match self {
            Self::Admin => UserType::Admin,
            Self::User => UserType::User,
            Self::A2a => UserType::A2a,
            Self::Mcp => UserType::Mcp,
            Self::Service | Self::HookGovern | Self::HookTrack => UserType::Service,
            Self::Anonymous => UserType::Anon,
        }
    }

    pub const fn from_user_type(user_type: UserType) -> Self {
        match user_type {
            UserType::Admin => Self::Admin,
            UserType::User => Self::User,
            UserType::A2a => Self::A2a,
            UserType::Mcp => Self::Mcp,
            UserType::Service => Self::Service,
            UserType::Anon | UserType::Unknown => Self::Anonymous,
        }
    }

    pub const fn is_user_role(&self) -> bool {
        matches!(self, Self::Admin | Self::User | Self::Anonymous)
    }

    pub const fn is_service_scope(&self) -> bool {
        matches!(
            self,
            Self::A2a | Self::Mcp | Self::Service | Self::HookGovern | Self::HookTrack
        )
    }

    pub const fn is_hook_scope(&self) -> bool {
        matches!(self, Self::HookGovern | Self::HookTrack)
    }

    pub const fn hierarchy_level(&self) -> u8 {
        match self {
            Self::Admin => 100,
            Self::User => 50,
            Self::Service => 40,
            Self::A2a => 30,
            Self::Mcp => 20,
            Self::HookGovern | Self::HookTrack => 15,
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
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "anonymous" => Ok(Self::Anonymous),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "service" => Ok(Self::Service),
            "hook:govern" => Ok(Self::HookGovern),
            "hook:track" => Ok(Self::HookTrack),
            _ => Err(ParseEnumError::new("permission", s)),
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

pub fn parse_permissions(s: &str) -> Result<Vec<Permission>, ParseEnumError> {
    s.split_whitespace().map(Permission::from_str).collect()
}
