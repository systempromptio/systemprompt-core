//! From an admitted identity to a local user: find the federated link, or
//! provision one when the policy allows, then refuse anyone who may not sign
//! in interactively and bring the user's roles in line with the policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_traits::auth::UserProvider;
use systemprompt_traits::sso::{
    ExternalIdentity, GatedIdentity, SsoError, SsoProvider, SsoRefusal,
};

/// The local user an SSO sign-in resolved to, and whether this sign-in
/// created the link.
#[derive(Debug, Clone)]
pub struct ResolvedUser {
    pub user_id: UserId,
    pub created: bool,
}

pub(super) async fn resolve_user(
    users: &dyn UserProvider,
    provider: &dyn SsoProvider,
    identity: &ExternalIdentity,
    gated: &GatedIdentity,
) -> Result<ResolvedUser, SsoError> {
    let issuer = provider.issuer();
    let existing = users
        .find_federated(issuer, &gated.external_sub)
        .await
        .map_err(SsoError::internal)?;
    let resolved = match existing {
        Some(user_id) => ResolvedUser {
            user_id,
            created: false,
        },
        None if provider.policy().auto_provision => {
            let claims = FederatedIdentityClaims {
                email: Some(gated.email.clone()),
                email_verified: identity.email_verified,
                name: Some(gated.display_name.clone()),
                preferred_username: None,
                roles: gated.roles.clone(),
            };
            let user_id = users
                .find_or_create_federated(issuer, &gated.external_sub, &claims)
                .await
                .map_err(SsoError::internal)?;
            ResolvedUser {
                user_id,
                created: true,
            }
        },
        None => return Err(SsoError::Refused(SsoRefusal::NotProvisioned)),
    };

    let user = users
        .find_by_id(&resolved.user_id)
        .await
        .map_err(SsoError::internal)?
        .ok_or_else(|| SsoError::internal("federated link points at a missing user"))?;
    if !user.is_active || user.ensure_interactive().is_err() {
        return Err(SsoError::Refused(SsoRefusal::NotProvisioned));
    }
    users
        .assign_roles(&resolved.user_id, &gated.roles)
        .await
        .map_err(SsoError::internal)?;
    Ok(resolved)
}
