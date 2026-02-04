use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JwtAudience {
    Web,
    Api,
    A2a,
    Mcp,
    Internal,
    #[serde(untagged)]
    Resource(String),
}

impl JwtAudience {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::A2a => "a2a",
            Self::Mcp => "mcp",
            Self::Internal => "internal",
            Self::Resource(s) => s.as_str(),
        }
    }

    pub fn standard() -> Vec<Self> {
        vec![Self::Web, Self::Api, Self::A2a, Self::Mcp]
    }

    pub fn service() -> Vec<Self> {
        vec![Self::Api, Self::Mcp, Self::A2a, Self::Internal]
    }
}

impl fmt::Display for JwtAudience {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for JwtAudience {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "web" => Ok(Self::Web),
            "api" => Ok(Self::Api),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "internal" => Ok(Self::Internal),
            _ => Ok(Self::Resource(s.to_string())),
        }
    }
}

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
}

impl fmt::Display for UserType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for UserType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "service" => Ok(Self::Service),
            "anon" => Ok(Self::Anon),
            "unknown" => Ok(Self::Unknown),
            _ => Err(anyhow!("Invalid user type: {s}")),
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
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "a2a" => Ok(Self::A2a),
            "mcp" => Ok(Self::Mcp),
            "service" => Ok(Self::Service),
            "anon" => Ok(Self::Anon),
            _ => Err(anyhow!("Invalid rate limit tier: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
    Anonymous,
}

impl UserRole {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::Anonymous => "anonymous",
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for UserRole {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "anonymous" => Ok(Self::Anonymous),
            _ => Err(anyhow!("Invalid user role: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
    Pending,
    Deleted,
    Temporary,
}

impl UserStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Suspended => "suspended",
            Self::Pending => "pending",
            Self::Deleted => "deleted",
            Self::Temporary => "temporary",
        }
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for UserStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "suspended" => Ok(Self::Suspended),
            "pending" => Ok(Self::Pending),
            "deleted" => Ok(Self::Deleted),
            "temporary" => Ok(Self::Temporary),
            _ => Err(anyhow!("Invalid user status: {s}")),
        }
    }
}
