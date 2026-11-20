//! Resolving the authenticated caller into the user the document service
//! authorizes: the account's current roles, read from the user provider so
//! a role change applies to the next request; and the author a REST write
//! records, attributed to the actor the request was made by (a person, or an
//! agent acting for the signed-in user), to the request's session, trace and
//! task, to the agent and OAuth client of the token, and to the conversation
//! named by an `x-context-id` header, which must be one the caller owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_files::documents::models::RevisionOrigin;
use systemprompt_files::documents::service::{Author, CommentOrigin};
use systemprompt_identifiers::{ContextId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ActorKind;
use systemprompt_runtime::documents::actor::request_actor;
use systemprompt_runtime::documents::attribution::{comment_origin, request_author};
use systemprompt_traits::{AuthUser, UserProvider};

use super::DocumentsState;
use super::error::DocumentsResult;
use crate::kernel::ApiHttpError;

pub(crate) struct Caller {
    pub(crate) user: AuthenticatedUser,
    pub(crate) trace: TraceId,
    pub(crate) actor: DocumentActor,
}

pub(super) async fn caller(
    state: &DocumentsState,
    req_ctx: &RequestContext,
) -> DocumentsResult<Caller> {
    let user = load(state.users.as_ref(), req_ctx.user_id()).await?;
    Ok(Caller {
        user,
        trace: req_ctx.trace_id().clone(),
        actor: request_actor(req_ctx),
    })
}

pub(super) async fn load(
    users: &dyn UserProvider,
    id: &UserId,
) -> DocumentsResult<AuthenticatedUser> {
    let found = users.find_by_id(id).await?;
    match found {
        Some(user) if user.is_active => Ok(authenticated(user)),
        _ => Err(ApiHttpError::unauthorized("account is not active").into()),
    }
}

pub(crate) fn authenticated(user: AuthUser) -> AuthenticatedUser {
    AuthenticatedUser::from_provider_user(user)
}

const fn actor_kind(actor: &DocumentActor) -> ActorKind {
    match actor {
        DocumentActor::Human => ActorKind::User,
        DocumentActor::Agent(_) => ActorKind::Agent,
        DocumentActor::System { .. } => ActorKind::System,
    }
}

pub(super) async fn api_author(
    state: &DocumentsState,
    ctx: &RequestContext,
    headers: &HeaderMap,
) -> DocumentsResult<Author> {
    let claimed = header_context(headers)?;
    let kind = actor_kind(&request_actor(ctx));
    Ok(request_author(
        state.contexts.as_ref(),
        ctx,
        (kind, RevisionOrigin::Api),
        claimed.as_ref(),
    )
    .await?)
}

pub(super) async fn api_comment_origin(
    state: &DocumentsState,
    ctx: &RequestContext,
    headers: &HeaderMap,
) -> DocumentsResult<CommentOrigin> {
    let claimed = header_context(headers)?;
    Ok(comment_origin(state.contexts.as_ref(), ctx, claimed.as_ref()).await?)
}

fn header_context(headers: &HeaderMap) -> DocumentsResult<Option<ContextId>> {
    let Some(value) = headers.get(systemprompt_identifiers::headers::CONTEXT_ID) else {
        return Ok(None);
    };
    let raw = value.to_str()?;
    if raw.is_empty() {
        return Ok(None);
    }
    Ok(Some(ContextId::try_new(raw)?))
}
