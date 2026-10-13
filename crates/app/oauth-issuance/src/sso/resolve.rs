//! From an admitted identity to a local user: find the federated link, link
//! an existing account by verified email when the policy allows, or provision
//! one, then refuse anyone who may not sign in interactively and settle the
//! user's roles through the registered [`super::RoleResolver`].
//!
//! The lookup order is fixed by [`plan_resolution`], a pure decision kept
//! apart from the I/O so it can be tested without a database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_traits::FederatedIdentityClaims;
use systemprompt_traits::auth::{AuthUser, UserProvider};
use systemprompt_traits::sso::{
    ExternalIdentity, GatedIdentity, SsoError, SsoIdentityPolicy, SsoProvider, SsoRefusal,
};

use super::roles::RoleResolver;

/// The local user an SSO sign-in resolved to, whether this sign-in created
/// the link, and the roles the [`super::RoleResolver`] settled on.
#[derive(Debug, Clone)]
pub struct ResolvedUser {
    pub user_id: UserId,
    pub created: bool,
    pub roles: Vec<String>,
}

/// The next step for an identity, decided from what the lookups found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionPlan {
    Existing(UserId),
    LinkExisting(UserId),
    Provision,
    Refuse,
}

#[must_use]
pub fn plan_resolution(
    federated: Option<UserId>,
    email_owner: Option<&AuthUser>,
    email_verified: bool,
    policy: &SsoIdentityPolicy,
) -> ResolutionPlan {
    if let Some(user_id) = federated {
        return ResolutionPlan::Existing(user_id);
    }
    if policy.link_by_email
        && email_verified
        && let Some(owner) = email_owner
        && owner.is_active
        && owner.ensure_interactive().is_ok()
    {
        return ResolutionPlan::LinkExisting(owner.id.clone());
    }
    if policy.auto_provision {
        ResolutionPlan::Provision
    } else {
        ResolutionPlan::Refuse
    }
}

pub(super) async fn resolve_user(
    users: &dyn UserProvider,
    provider: &dyn SsoProvider,
    roles: &dyn RoleResolver,
    identity: &ExternalIdentity,
    gated: &GatedIdentity,
) -> Result<ResolvedUser, SsoError> {
    let issuer = provider.issuer();
    let policy = provider.policy();
    let federated = users
        .find_federated(issuer, &gated.external_sub)
        .await
        .map_err(SsoError::internal)?;
    let email_owner = if federated.is_none() && policy.link_by_email && identity.email_verified {
        users
            .find_by_email(&gated.email)
            .await
            .map_err(SsoError::internal)?
    } else {
        None
    };

    let (user_id, created) = match plan_resolution(
        federated,
        email_owner.as_ref(),
        identity.email_verified,
        policy,
    ) {
        ResolutionPlan::Existing(user_id) => (user_id, false),
        ResolutionPlan::LinkExisting(user_id) => {
            users
                .link_federated(issuer, &gated.external_sub, &user_id)
                .await
                .map_err(SsoError::internal)?;
            (user_id, false)
        },
        ResolutionPlan::Provision => {
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
            (user_id, true)
        },
        ResolutionPlan::Refuse => return Err(SsoError::Refused(SsoRefusal::NotProvisioned)),
    };

    let user = users
        .find_by_id(&user_id)
        .await
        .map_err(SsoError::internal)?
        .ok_or_else(|| SsoError::internal("federated link points at a missing user"))?;
    if !user.is_active || user.ensure_interactive().is_err() {
        return Err(SsoError::Refused(SsoRefusal::NotProvisioned));
    }
    let resolved_roles = roles.resolve(&user_id, &gated.roles).await?;
    users
        .assign_roles(&user_id, &resolved_roles)
        .await
        .map_err(SsoError::internal)?;
    Ok(ResolvedUser {
        user_id,
        created,
        roles: resolved_roles,
    })
}
