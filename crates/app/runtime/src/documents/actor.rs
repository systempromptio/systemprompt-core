//! The document actor of a request, derived from its credential alone.
//!
//! Only a first-party browser session cookie is the person: a bearer token
//! of any kind (a bridge JWT, a PAT, an API or MCP token, an agent run's
//! token, one carrying a purpose or a delegation chain) is an agent acting
//! for them. The agent's name and whether it is attended come from signed
//! claims, never from request metadata: the name is the token's `purpose`
//! (an agent run's `farm:<run>`), else its `client_id`, else `agent`;
//! `attended` is `true` only for a token whose `interaction` claim is
//! `interactive`, which only the exchange-code bridge host flow mints, and
//! never for one carrying a `purpose` (a service account's, a broker's or
//! an agent run's), whatever else it claims.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use systemprompt_files::documents::agents::{DocumentActor, UNNAMED_AGENT, act_as};
use systemprompt_models::auth::TokenInteraction;
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};

const MAX_AGENT_NAME: usize = 64;

/// Runs `work` as the actor `ctx` proves: the one call an extension makes
/// around document service calls it serves from its own routes.
pub async fn act_as_request<F: Future>(ctx: &RequestContext, work: F) -> F::Output {
    act_as(request_actor(ctx), work).await
}

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
    let attended =
        interaction == Some(UserInteractionMode::Interactive) && ctx.caller_purpose().is_none();
    DocumentActor::agent(signed_agent_name(ctx), attended)
}

/// The agent a bearer token acts as, from its verified claims: for a
/// surface that verifies the token itself (the bridge Drive routes).
#[must_use]
pub fn bearer_actor(
    purpose: Option<&str>,
    client_id: Option<&str>,
    interaction: Option<TokenInteraction>,
) -> DocumentActor {
    let attended = interaction == Some(TokenInteraction::Interactive) && purpose.is_none();
    DocumentActor::agent(agent_name(purpose, client_id), attended)
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
