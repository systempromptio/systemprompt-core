//! The document actor of a request, derived from its credential alone.
//!
//! Only a first-party browser session cookie is the person: a bearer token
//! of any kind (a bridge JWT, a PAT, an API or MCP token, an agent run's
//! token, one carrying a purpose or a delegation chain) is an agent acting
//! for them. The agent's name and whether it is attended come from signed
//! claims, never from request metadata: the name is the token's `purpose`
//! (an agent run's `farm:<run>`), else its `client_id`, else `agent`;
//! `attended` is `true` only for a token whose `interaction` claim is
//! `interactive`, which only the exchange-code bridge host flow mints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::agents::{DocumentActor, UNNAMED_AGENT};
use systemprompt_models::auth::TokenInteraction;
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};

const MAX_AGENT_NAME: usize = 64;

/// The actor document calls made for `ctx` run as.
#[must_use]
pub fn request_actor(ctx: &RequestContext) -> DocumentActor {
    let interaction = ctx.settings.user_interaction_mode;
    let person = ctx.is_browser_session()
        && ctx.caller_purpose().is_none()
        && ctx.act_chain().is_empty()
        && interaction != Some(UserInteractionMode::NonInteractive);
    if person {
        return DocumentActor::Human;
    }
    DocumentActor::agent(
        signed_agent_name(ctx),
        interaction == Some(UserInteractionMode::Interactive),
    )
}

/// The agent a bearer token acts as, from its verified claims: for a
/// surface that verifies the token itself (the bridge Drive routes).
#[must_use]
pub fn bearer_actor(
    purpose: Option<&str>,
    client_id: Option<&str>,
    interaction: Option<TokenInteraction>,
) -> DocumentActor {
    DocumentActor::agent(
        agent_name(purpose, client_id),
        interaction == Some(TokenInteraction::Interactive),
    )
}

/// The agent's name from signed claims: at most 64 chars of letters,
/// digits, `-`, `_`, `.`, `:` or `/`, else `agent`.
#[must_use]
pub fn signed_agent_name(ctx: &RequestContext) -> String {
    agent_name(
        ctx.caller_purpose(),
        ctx.client_id().map(|client| client.as_str()),
    )
}

fn agent_name(purpose: Option<&str>, client_id: Option<&str>) -> String {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/');
    let valid =
        |name: &str| !name.is_empty() && name.len() <= MAX_AGENT_NAME && name.chars().all(allowed);
    purpose
        .or(client_id)
        .filter(|name| valid(name))
        .unwrap_or(UNNAMED_AGENT)
        .to_owned()
}
