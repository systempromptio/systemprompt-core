//! From an admitted identity to a local user: resolve it through the one
//! federated resolution ([`UserProvider::resolve_federated`]) under the
//! provider's link policy — which also refuses anyone who may not sign in
//! interactively — and settle roles through the registered
//! [`super::RoleResolver`] — only on an account the provider itself
//! provisioned. A pre-existing account linked by email keeps the roles it
//! already had.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_traits::auth::{AuthProviderError, UserProvider};
use systemprompt_traits::sso::{ExternalIdentity, GatedIdentity, SsoError, SsoProvider};
use systemprompt_traits::{FederatedIdentityClaims, FederatedLink};

use super::roles::RoleResolver;

/// The local user an SSO sign-in resolved to, whether this sign-in created
/// the account, and the roles it holds after the sign-in.
#[derive(Debug, Clone)]
pub struct ResolvedUser {
    pub user_id: UserId,
    pub created: bool,
    pub roles: Vec<String>,
}

pub(super) async fn resolve_user(
    users: &dyn UserProvider,
    provider: &dyn SsoProvider,
    roles: &dyn RoleResolver,
    identity: &ExternalIdentity,
    gated: &GatedIdentity,
) -> Result<ResolvedUser, SsoError> {
    let claims = FederatedIdentityClaims {
        email: Some(gated.email.clone()),
        email_verified: identity.email_verified,
        name: Some(gated.display_name.clone()),
        preferred_username: None,
        roles: gated.roles.clone(),
    };
    let resolution = users
        .resolve_federated(
            provider.issuer(),
            &gated.external_sub,
            &claims,
            &provider.policy().link_policy(),
        )
        .await
        .map_err(refused_or_internal)?;

    let user = users
        .find_by_id(&resolution.user_id)
        .await
        .map_err(SsoError::internal)?
        .ok_or_else(|| SsoError::internal("federated link points at a missing user"))?;
    let held_roles = if resolution.link.issuer_owns_roles() {
        let resolved = roles.resolve(&resolution.user_id, &gated.roles).await?;
        users
            .assign_roles(&resolution.user_id, &resolved)
            .await
            .map_err(SsoError::internal)?;
        resolved
    } else {
        user.roles
    };
    Ok(ResolvedUser {
        user_id: resolution.user_id,
        created: resolution.link == FederatedLink::Provisioned,
        roles: held_roles,
    })
}

fn refused_or_internal(err: AuthProviderError) -> SsoError {
    err.federated_refusal().map_or_else(
        || SsoError::internal(err),
        |refusal| SsoError::Refused(refusal.into()),
    )
}
