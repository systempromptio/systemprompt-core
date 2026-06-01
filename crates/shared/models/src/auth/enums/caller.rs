//! Caller classification and the rate-limit tier it maps to.
//!
//! [`UserType`] is the privilege class derived from a permission set;
//! [`RateLimitTier`] is the throughput band it resolves to; [`TokenType`]
//! is the bearer-scheme marker. [`UserType::from_permissions`] is the single
//! source of truth for the permission → type mapping.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::auth::permission::Permission;
use crate::errors::ParseEnumError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserType {
    Admin,
    User,
    A2a,
    Mcp,
    Service,
    Anon,
    Unknown,
}

impl UserType {
    /// Derives the caller type from a permission set, the single source of
    /// truth for the permission → type mapping. The precedence is
    /// privilege-descending (`Admin` wins over `User`, etc.); the hook scopes
    /// resolve to `Service` so a hook principal is never silently downgraded
    /// to `Anon`.
    pub fn from_permissions(permissions: &[Permission]) -> Self {
        let has = |p: Permission| permissions.contains(&p);
        if has(Permission::Admin) {
            Self::Admin
        } else if has(Permission::User) {
            Self::User
        } else if has(Permission::A2a) {
            Self::A2a
        } else if has(Permission::Mcp) {
            Self::Mcp
        } else if has(Permission::Service)
            || has(Permission::HookGovern)
            || has(Permission::HookTrack)
        {
            Self::Service
        } else {
            Self::Anon
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Service => "service",
            Self::Anon => "anon",
            Self::Unknown => "unknown",
        }
    }

    pub const fn rate_tier(&self) -> RateLimitTier {
        match self {
            Self::Admin => RateLimitTier::Admin,
            Self::User => RateLimitTier::User,
            Self::A2a => RateLimitTier::A2a,
            Self::Mcp => RateLimitTier::Mcp,
            Self::Service => RateLimitTier::Service,
            Self::Anon | Self::Unknown => RateLimitTier::Anon,
        }
    }

    // Human types (Admin/User) are authoritative on the users row, not the JWT:
    // an Admin-claimed token whose user row is no longer in the admin role gets
    // downgraded here. Machine types (Service/A2a/Mcp/Anon) are not reflected in
    // users.roles — they are minted by the OAuth layer and trusted as claimed.
    #[must_use]
    pub const fn reconcile_with(self, user_is_admin: bool) -> Self {
        match self {
            Self::Admin if !user_is_admin => Self::User,
            other => other,
        }
    }
}

impl fmt::Display for UserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for UserType {
    type Err = ParseEnumError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "service" => Ok(Self::Service),
            "anon" => Ok(Self::Anon),
            "unknown" => Ok(Self::Unknown),
            _ => Err(ParseEnumError::new("user_type", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    #[default]
    Bearer,
}

impl TokenType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bearer => "Bearer",
        }
    }
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bearer")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitTier {
    Admin,
    User,
    A2a,
    Mcp,
    Service,
    Anon,
}

impl RateLimitTier {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Service => "service",
            Self::Anon => "anon",
        }
    }
}

impl fmt::Display for RateLimitTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RateLimitTier {
    type Err = ParseEnumError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "service" => Ok(Self::Service),
            "anon" => Ok(Self::Anon),
            _ => Err(ParseEnumError::new("rate_limit_tier", s)),
        }
    }
}
