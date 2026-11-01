//! The lifecycle status of a fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a fact stands: only `Confirmed` facts are recalled; `Stale` ones
/// only when the profile allows a marked injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FactStatus {
    Candidate,
    Proposed,
    Confirmed,
    Disputed,
    Stale,
    Superseded,
    Retired,
}

/// A string that is not a [`FactStatus`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown fact status {0:?}")]
pub struct UnknownFactStatus(pub String);

impl FactStatus {
    pub const ALL: [Self; 7] = [
        Self::Candidate,
        Self::Proposed,
        Self::Confirmed,
        Self::Disputed,
        Self::Stale,
        Self::Superseded,
        Self::Retired,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Proposed => "proposed",
            Self::Confirmed => "confirmed",
            Self::Disputed => "disputed",
            Self::Stale => "stale",
            Self::Superseded => "superseded",
            Self::Retired => "retired",
        }
    }

    #[must_use]
    pub const fn is_recallable(self) -> bool {
        matches!(self, Self::Confirmed)
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Superseded | Self::Retired)
    }
}

impl fmt::Display for FactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FactStatus {
    type Err = UnknownFactStatus;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == s)
            .ok_or_else(|| UnknownFactStatus(s.to_owned()))
    }
}
