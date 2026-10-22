//! The traffic class a session is counted under: the vocabulary of
//! `session_traffic_rollup_hourly.traffic_class`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficClass {
    Human,
    Ghost,
    Bot,
}

impl TrafficClass {
    pub const ALL: [Self; 3] = [Self::Human, Self::Ghost, Self::Bot];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Ghost => "ghost",
            Self::Bot => "bot",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.as_str() == value)
    }
}

impl std::fmt::Display for TrafficClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
