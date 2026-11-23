//! The actor of a request, derived from its credential alone.
//!
//! A bearer token of any kind is an agent acting for its user. The agent's
//! name comes from signed claims, never from request metadata: the token's
//! `purpose` (an agent run's `farm:<run>`), else its `client_id`, else
//! [`UNNAMED_AGENT`]; a name that is not a valid [`AgentName`] (over 64 bytes,
//! or a character outside `[A-Za-z0-9-_.:/]`) falls back the same way.
//! `attended` is `true` only for a token whose `interaction` claim is
//! `interactive` and that carries no `purpose`.
//!
//! [`caller_actor`] reads the actor of an authenticated request: a person in
//! a browser or UI session with no caller purpose, no delegation chain and no
//! non-interactive mode is [`RequestActor::Human`]; every other caller is the
//! agent its signed claims name, attended only in interactive mode with no
//! caller purpose.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AgentName, ClientId};

use super::claims::TokenInteraction;
use crate::execution::context::{RequestContext, UserInteractionMode};

pub const UNNAMED_AGENT: &str = "agent";

/// Who a request acts as: the person, or an agent acting for them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestActor {
    Human,
    Agent { name: AgentName, attended: bool },
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
pub fn agent_name(purpose: Option<&str>, client_id: Option<&ClientId>) -> AgentName {
    purpose
        .or_else(|| client_id.map(ClientId::as_str))
        .and_then(|name| AgentName::try_new(name).ok())
        .unwrap_or_else(|| AgentName::new(UNNAMED_AGENT))
}

#[must_use]
pub fn caller_actor(ctx: &RequestContext) -> RequestActor {
    let interaction = ctx.settings.user_interaction_mode;
    let person = (ctx.is_browser_session() || ctx.is_ui_session())
        && ctx.caller_purpose().is_none()
        && ctx.act_chain().is_empty()
        && interaction != Some(UserInteractionMode::NonInteractive);
    if person {
        return RequestActor::Human;
    }
    let attended =
        interaction == Some(UserInteractionMode::Interactive) && ctx.caller_purpose().is_none();
    RequestActor::Agent {
        name: agent_name(ctx.caller_purpose(), ctx.client_id()),
        attended,
    }
}
