//! Resolving the authenticated caller into the user the document service
//! authorizes: the account's current roles, read from the user provider so
//! a role change applies to the next request; and the author a REST write
//! records, attributed to the request's session, trace and task and to the
//! conversation named by an `x-context-id` header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_files::documents::models::RevisionAttribution;
use systemprompt_files::documents::service::{Author, RevisionOrigin};
use systemprompt_identifiers::{ContextId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, BaseRoles, Permission};
use systemprompt_models::documents::ActorKind;
use systemprompt_runtime::documents::actor::request_actor;
use systemprompt_traits::{AuthUser, UserProvider};

use super::DocumentsState;
use super::error::DocumentsResult;
use crate::error::ApiHttpError;

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
    let permission = if user.roles.iter().any(|role| role == BaseRoles::ADMIN) {
        Permission::Admin
    } else {
        Permission::User
    };
    AuthenticatedUser::new_with_roles(user.id, user.name, user.email, vec![permission], user.roles)
}

pub(super) fn api_author(ctx: &RequestContext, headers: &HeaderMap) -> Author {
    let present = |value: &str| !value.is_empty();
    Author {
        actor_kind: ActorKind::User,
        origin: RevisionOrigin::Api,
        attribution: RevisionAttribution {
            session_id: Some(ctx.session_id().clone()).filter(|id| present(id.as_str())),
            context_id: headers
                .get(systemprompt_identifiers::headers::CONTEXT_ID)
                .and_then(|value| value.to_str().ok())
                .and_then(context_id),
            trace_id: Some(ctx.trace_id().clone()).filter(|id| present(id.as_str())),
            task_id: ctx.task_id().cloned(),
            on_behalf_of: None,
        },
    }
}

fn context_id(raw: &str) -> Option<ContextId> {
    match ContextId::try_new(raw) {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(error = %error, "x-context-id is not a context id; revision carries none");
            None
        },
    }
}
