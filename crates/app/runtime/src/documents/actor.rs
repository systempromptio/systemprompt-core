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

use systemprompt_files::documents::agents::{DocumentActor, act_as};
use systemprompt_identifiers::{AgentName, ClientId};
use systemprompt_models::auth::{self, RequestActor, TokenInteraction};
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};

pub async fn act_as_request<F: Future>(ctx: &RequestContext, work: F) -> F::Output {
    act_as(request_actor(ctx), work).await
}

#[must_use]
pub fn request_actor(ctx: &RequestContext) -> DocumentActor {
    document_actor(caller_actor(ctx))
}

#[must_use]
pub fn caller_actor(ctx: &RequestContext) -> RequestActor {
    let interaction = ctx.settings.user_interaction_mode;
    let person = ctx.is_browser_session()
        && ctx.caller_purpose().is_none()
        && ctx.act_chain().is_empty()
        && interaction != Some(UserInteractionMode::NonInteractive);
    if person {
        return RequestActor::Human;
    }
    let attended =
        interaction == Some(UserInteractionMode::Interactive) && ctx.caller_purpose().is_none();
    RequestActor::Agent {
        name: signed_agent_name(ctx),
        attended,
    }
}

#[must_use]
pub fn bearer_actor(
    purpose: Option<&str>,
    client: Option<&ClientId>,
    interaction: Option<TokenInteraction>,
) -> DocumentActor {
    document_actor(auth::bearer_actor(purpose, client, interaction))
}

#[must_use]
pub fn signed_agent_name(ctx: &RequestContext) -> AgentName {
    auth::agent_name(ctx.caller_purpose(), ctx.client_id())
}

fn document_actor(actor: RequestActor) -> DocumentActor {
    match actor {
        RequestActor::Human => DocumentActor::Human,
        RequestActor::Agent { name, attended } => DocumentActor::agent(name, attended),
    }
}
