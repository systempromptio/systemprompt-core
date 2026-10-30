//! Who a fact is shared with: one user, a project, a group or the whole
//! organisation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

/// The level of a [`FactScope`], as profiles and recall filters name it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ScopeLevel {
    User,
    Project,
    Group,
    Org,
}

/// A string that is not a [`ScopeLevel`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown fact scope {0:?}")]
pub struct UnknownScopeLevel(pub String);

impl ScopeLevel {
    pub const ALL: [Self; 4] = [Self::User, Self::Project, Self::Group, Self::Org];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Group => "group",
            Self::Org => "org",
        }
    }
}

impl fmt::Display for ScopeLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ScopeLevel {
    type Err = UnknownScopeLevel;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|level| level.as_str() == s)
            .ok_or_else(|| UnknownScopeLevel(s.to_owned()))
    }
}

/// The audience a fact is scoped to, with the key that names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "level", content = "key", rename_all = "snake_case")]
pub enum FactScope {
    User(UserId),
    Project(String),
    Group(String),
    Org,
}

impl FactScope {
    #[must_use]
    pub const fn level(&self) -> ScopeLevel {
        match self {
            Self::User(_) => ScopeLevel::User,
            Self::Project(_) => ScopeLevel::Project,
            Self::Group(_) => ScopeLevel::Group,
            Self::Org => ScopeLevel::Org,
        }
    }

    #[must_use]
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::User(user) => Some(user.as_str()),
            Self::Project(key) | Self::Group(key) => Some(key),
            Self::Org => None,
        }
    }
}
