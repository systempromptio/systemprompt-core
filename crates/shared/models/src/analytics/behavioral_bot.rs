//! The one behavioural-bot classifier: every reason a session is stamped
//! `is_behavioral_bot`, with the score each reason contributes.
//!
//! `user_sessions.behavioral_bot_reason` stores [`BehavioralBotReason::as_str`]
//! and `behavioral_bot_score` stores [`BehavioralBotReason::score`]; the
//! repository that writes the flag and the detector that sums signals both
//! read the numbers from here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehavioralBotReason {
    RequestCountExceeded,
    GhostSession,
    NoJavascript,
}

impl BehavioralBotReason {
    pub const ALL: [Self; 3] = [
        Self::RequestCountExceeded,
        Self::GhostSession,
        Self::NoJavascript,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestCountExceeded => "request_count_exceeded",
            Self::GhostSession => "ghost_session",
            Self::NoJavascript => "no_javascript",
        }
    }

    #[must_use]
    pub const fn score(self) -> i32 {
        match self {
            Self::RequestCountExceeded => 30,
            Self::GhostSession => 35,
            Self::NoJavascript => 20,
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

impl std::fmt::Display for BehavioralBotReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
