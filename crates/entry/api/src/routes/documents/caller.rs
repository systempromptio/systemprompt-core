//! Resolving the authenticated caller into the user the document service
//! authorizes: the account's current roles, read from the user provider so
//! a role change applies to the next request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, BaseRoles, Permission};
use systemprompt_traits::{AuthUser, UserProvider};

use super::DocumentsState;
use super::error::DocumentsResult;
use crate::error::ApiHttpError;

pub(crate) struct Caller {
    pub(crate) user: AuthenticatedUser,
    pub(crate) trace: TraceId,
}

pub(super) async fn caller(
    state: &DocumentsState,
    req_ctx: &RequestContext,
) -> DocumentsResult<Caller> {
    let user = load(state.users.as_ref(), req_ctx.user_id()).await?;
    Ok(Caller {
        user,
        trace: req_ctx.trace_id().clone(),
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
