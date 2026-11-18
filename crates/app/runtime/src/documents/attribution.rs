//! The conversation a document write is attributed to, and the names of the
//! conversations a reader owns.
//!
//! A write names its conversation by a per-call `context_id` or by the
//! request's context (the `x-context-id` header). The one id trusted without
//! a lookup is the context the server derives from the caller's own session
//! (`ContextId::derived_from_session`), which an MCP connection that sends no
//! header is given. Any other id must be a conversation the calling user
//! owns: one they do not own and one that does not exist are both refused as
//! `unknown_context`, so the answer never tells a caller that another user's
//! conversation exists, and nothing is written. A lookup that fails refuses
//! the write as an internal failure rather than dropping the attribution.
//!
//! The agent recorded on a revision or comment is the request actor's signed
//! name and the client is the token's signed `client_id`; neither is ever
//! read from request metadata. A conversation's name is shown only to the
//! user who owns it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{RevisionAttribution, RevisionOrigin};
use systemprompt_files::documents::service::{Author, CommentOrigin};
use systemprompt_identifiers::{AgentName, ContextId, UserId};
use systemprompt_models::documents::ActorKind;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_traits::ContextDirectory;

use super::actor::request_actor;

pub async fn attributed_context(
    contexts: &dyn ContextDirectory,
    ctx: &RequestContext,
    claimed: Option<&ContextId>,
) -> Result<Option<ContextId>, DocumentsError> {
    let Some(claimed) = claimed.filter(|id| !id.as_str().is_empty()) else {
        return Ok(None);
    };
    let session = ctx.session_id();
    if !session.as_str().is_empty() && *claimed == ContextId::derived_from_session(session) {
        return Ok(Some(claimed.clone()));
    }
    let owned = contexts
        .owned_context(ctx.user_id(), claimed)
        .await
        .map_err(DocumentsError::Contexts)?;
    owned
        .map(|owned| Some(owned.context_id))
        .ok_or_else(|| DocumentsError::UnknownContext {
            context_id: claimed.clone(),
        })
}

fn signed_agent(ctx: &RequestContext) -> Option<AgentName> {
    request_actor(ctx)
        .as_agent()
        .map(|agent| agent.name.clone())
}

pub async fn request_author(
    contexts: &dyn ContextDirectory,
    ctx: &RequestContext,
    (actor_kind, origin): (ActorKind, RevisionOrigin),
    claimed: Option<&ContextId>,
) -> Result<Author, DocumentsError> {
    let present = |value: &str| !value.is_empty();
    let context_id = attributed_context(contexts, ctx, claimed).await?;
    let on_behalf_of = (actor_kind == ActorKind::Agent)
        .then(|| ctx.user_id().clone())
        .filter(|id| present(id.as_str()));
    Ok(Author {
        actor_kind,
        origin,
        attribution: RevisionAttribution {
            session_id: Some(ctx.session_id().clone()).filter(|id| present(id.as_str())),
            context_id,
            trace_id: Some(ctx.trace_id().clone()).filter(|id| present(id.as_str())),
            task_id: ctx.task_id().cloned(),
            on_behalf_of,
            agent_name: signed_agent(ctx),
            client_id: ctx.client_id().cloned(),
        },
    })
}

pub async fn comment_origin(
    contexts: &dyn ContextDirectory,
    ctx: &RequestContext,
    claimed: Option<&ContextId>,
) -> Result<CommentOrigin, DocumentsError> {
    Ok(CommentOrigin {
        via_agent: signed_agent(ctx),
        context_id: attributed_context(contexts, ctx, claimed).await?,
        client_id: ctx.client_id().cloned(),
    })
}

pub fn edit_origin(ctx: &RequestContext) -> CommentOrigin {
    CommentOrigin {
        via_agent: signed_agent(ctx),
        ..CommentOrigin::default()
    }
}

pub async fn owned_context_names<'a>(
    contexts: &dyn ContextDirectory,
    viewer: &UserId,
    ids: impl IntoIterator<Item = &'a ContextId>,
) -> Result<HashMap<ContextId, String>, DocumentsError> {
    let mut unique: Vec<ContextId> = Vec::new();
    for id in ids {
        if !unique.contains(id) {
            unique.push(id.clone());
        }
    }
    if unique.is_empty() {
        return Ok(HashMap::new());
    }
    contexts
        .owned_names(viewer, &unique)
        .await
        .map_err(DocumentsError::Contexts)
}
