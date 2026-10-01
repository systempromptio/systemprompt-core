//! The closed set of host applications the bridge integrates with.
//!
//! [`HostKind`] is the one definition of host identity shared by the gateway
//! (which advertises the hosts it serves and attests their traffic) and the
//! bridge (which enrols, syncs and mints loopback credentials per host). Its
//! wire form is the kebab-case host id (`claude-code`, `codex-cli`, …), the
//! same string every bridge config file and loopback token scope already
//! carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostKind {
    ClaudeCode,
    ClaudeDesktop,
    CodexCli,
    Hermes,
    #[serde(rename = "opencode")]
    OpenCode,
}

impl HostKind {
    pub const ALL: [Self; 5] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::CodexCli,
        Self::Hermes,
        Self::OpenCode,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::CodexCli => "codex-cli",
            Self::Hermes => "hermes",
            Self::OpenCode => "opencode",
        }
    }
}

impl fmt::Display for HostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown host id `{0}`")]
pub struct UnknownHostKind(pub String);

impl FromStr for HostKind {
    type Err = UnknownHostKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == s)
            .ok_or_else(|| UnknownHostKind(s.to_owned()))
    }
}
