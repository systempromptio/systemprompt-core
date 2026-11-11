//! Which caller may manage a registered OAuth client: its owner or an
//! admin. Any other caller is answered exactly as for a client that does not
//! exist, so a client id never confirms another user's client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ClientId;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_oauth::OAuthClient;
use systemprompt_oauth::repository::{ClientOwnerScope, OAuthRepository};

use super::super::OAuthHttpError;

pub(super) fn listing_scope(req_ctx: &RequestContext) -> ClientOwnerScope {
    match req_ctx.user_type() {
        UserType::Admin => ClientOwnerScope::Every,
        _ => ClientOwnerScope::OwnedBy(req_ctx.user_id().clone()),
    }
}

fn may_manage(req_ctx: &RequestContext, client: &OAuthClient) -> bool {
    match req_ctx.user_type() {
        UserType::Admin => true,
        UserType::Anon => false,
        _ => client.owner_user_id == *req_ctx.user_id(),
    }
}

pub(super) async fn managed_client(
    repository: &OAuthRepository,
    req_ctx: &RequestContext,
    client_id: &ClientId,
) -> Result<OAuthClient, OAuthHttpError> {
    repository
        .find_client_by_id(client_id)
        .await?
        .filter(|client| may_manage(req_ctx, client))
        .ok_or_else(|| OAuthHttpError::not_found(format!("Client with ID '{client_id}' not found")))
}
