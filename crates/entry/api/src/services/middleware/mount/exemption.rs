//! The closed list of core route groups allowed a group-level body limit
//! above [`DEFAULT_BODY_LIMIT_BYTES`], each named by its base path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::DEFAULT_BODY_LIMIT_BYTES;

/// A core route group, named by its base path, whose group-level body limit
/// is above [`DEFAULT_BODY_LIMIT_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreBodyExemption {
    AgentProxy,
}

impl CoreBodyExemption {
    pub const ALL: &'static [Self] = &[Self::AgentProxy];

    #[must_use]
    pub const fn base_path(self) -> &'static str {
        match self {
            Self::AgentProxy => systemprompt_models::modules::ApiPaths::AGENTS_BASE,
        }
    }

    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::AgentProxy => systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES,
        }
    }

    pub(super) fn admits(base_path: &str, declared: usize) -> bool {
        declared <= DEFAULT_BODY_LIMIT_BYTES
            || Self::ALL.iter().any(|exemption| {
                exemption.base_path() == base_path && exemption.bytes() == declared
            })
    }
}
