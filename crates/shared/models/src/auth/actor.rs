//! The actor of a request, derived from its credential alone.
//!
//! A bearer token of any kind is an agent acting for its user. The agent's
//! name comes from signed claims, never from request metadata: the token's
//! `purpose` (an agent run's `farm:<run>`), else its `client_id`, else
//! [`UNNAMED_AGENT`]; a name longer than [`MAX_AGENT_NAME`] or with a
//! character outside `[A-Za-z0-9-_.:/]` falls back the same way. `attended`
//! is `true` only for a token whose `interaction` claim is `interactive` and
//! that carries no `purpose`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ClientId;

use super::claims::TokenInteraction;

pub const UNNAMED_AGENT: &str = "agent";
pub const MAX_AGENT_NAME: usize = 64;

/// Who a request acts as: the person, or an agent acting for them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestActor {
    Human,
    Agent { name: String, attended: bool },
}

impl RequestActor {
    #[must_use]
    pub const fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    #[must_use]
    pub const fn is_unattended_agent(&self) -> bool {
        matches!(
            self,
            Self::Agent {
                attended: false,
                ..
            }
        )
    }
}

#[must_use]
pub fn bearer_actor(
    purpose: Option<&str>,
    client_id: Option<&ClientId>,
    interaction: Option<TokenInteraction>,
) -> RequestActor {
    let attended = interaction == Some(TokenInteraction::Interactive) && purpose.is_none();
    RequestActor::Agent {
        name: agent_name(purpose, client_id),
        attended,
    }
}

#[must_use]
pub fn agent_name(purpose: Option<&str>, client_id: Option<&ClientId>) -> String {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/');
    let valid =
        |name: &str| !name.is_empty() && name.len() <= MAX_AGENT_NAME && name.chars().all(allowed);
    purpose
        .or_else(|| client_id.map(ClientId::as_str))
        .filter(|name| valid(name))
        .unwrap_or(UNNAMED_AGENT)
        .to_owned()
}
