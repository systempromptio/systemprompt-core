//! Enterprise-Managed Authorization account linking.
//!
//! An ID-JAG asserts an enterprise identity — the employee an `IdP` has already
//! authorized for this resource. This module resolves that assertion to a local
//! account so the exchanged access token, its session, and its audit trail name
//! the employee rather than whichever principal owns the OAuth client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use systemprompt_identifiers::UserId;
use systemprompt_models::auth::Permission;
use systemprompt_traits::{AuthProviderError, FederatedIdentityClaims, LinkPolicy};

use crate::error::{OauthError, OauthResult};
use crate::state::OAuthState;

/// Enterprise identity asserted by an ID-JAG.
///
/// `email_verified` is carried verbatim from the issuer's claim and defaults
/// to `false`; an email is never treated as verified by its presence, because
/// verified-email linking binds the principal to an existing local account.
/// `link` is the asserting issuer's policy for exactly that.
#[derive(Debug, Clone)]
pub struct EnterprisePrincipal {
    pub issuer: String,
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub link: LinkPolicy,
}

impl EnterprisePrincipal {
    #[must_use]
    pub fn verified_claims(&self) -> FederatedIdentityClaims {
        FederatedIdentityClaims {
            email_verified: self.email_verified && self.email.is_some(),
            email: self.email.clone(),
            ..Default::default()
        }
    }
}

/// A local account an exchanged token may be issued for.
#[derive(Debug, Clone)]
pub struct LinkedSubject {
    pub user_id: UserId,
    pub name: String,
    pub email: String,
    pub permissions: Vec<Permission>,
}

pub async fn link_enterprise_principal(
    state: &OAuthState,
    principal: &EnterprisePrincipal,
) -> OauthResult<LinkedSubject> {
    let claims = principal.verified_claims();

    let user_id = match state
        .user_provider()
        .resolve_federated(&principal.issuer, &principal.sub, &claims, &principal.link)
        .await
    {
        Ok(resolution) => resolution.user_id,
        Err(err) => return Err(refused_or_failed(err)),
    };

    let user = state
        .user_provider()
        .find_by_id(&user_id)
        .await
        .map_err(|source| OauthError::UserProvider {
            context: "loading the linked ID-JAG subject",
            source,
        })?
        .ok_or(OauthError::Internal("linked ID-JAG subject vanished"))?;

    if !user.is_active {
        return Err(OauthError::InvalidGrant(
            "the enterprise identity is linked to an inactive account".to_owned(),
        ));
    }

    let permissions = user
        .roles
        .iter()
        .filter_map(|r| Permission::from_str(r).ok())
        .collect();

    Ok(LinkedSubject {
        user_id,
        name: user.name,
        email: user.email,
        permissions,
    })
}

fn refused_or_failed(err: AuthProviderError) -> OauthError {
    err.federated_refusal().map_or_else(
        || OauthError::UserProvider {
            context: "linking the ID-JAG subject to an account",
            source: err,
        },
        |refusal| {
            OauthError::InvalidGrant(format!(
                "the enterprise identity cannot be linked: {refusal}"
            ))
        },
    )
}
