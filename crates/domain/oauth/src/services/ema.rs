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
use systemprompt_traits::FederatedIdentityClaims;

use crate::error::{OauthError, OauthResult};
use crate::state::OAuthState;

/// Enterprise identity asserted by an ID-JAG.
#[derive(Debug, Clone)]
pub struct EnterprisePrincipal {
    pub issuer: String,
    pub sub: String,
    pub email: Option<String>,
}

/// A local account an exchanged token may be issued for.
#[derive(Debug, Clone)]
pub struct LinkedSubject {
    pub user_id: UserId,
    pub name: String,
    pub email: String,
    pub permissions: Vec<Permission>,
}

/// Link an ID-JAG's enterprise identity to a local account, provisioning one on
/// first sight.
///
/// The `email_verified` assertion the federated linker demands is satisfied by
/// the ID-JAG itself: it is RS256-signed by a `kid` resolved from a configured
/// trusted issuer's JWKS, and that issuer is separately marked
/// `can_issue_id_jag`. There is no weaker path into this function, so the email
/// is as trustworthy as an `email_verified` `id_token` claim.
pub async fn link_enterprise_principal(
    state: &OAuthState,
    principal: &EnterprisePrincipal,
) -> OauthResult<LinkedSubject> {
    let claims = FederatedIdentityClaims {
        email: principal.email.clone(),
        email_verified: principal.email.is_some(),
        ..Default::default()
    };

    let user_id = state
        .user_provider()
        .find_or_create_federated(&principal.issuer, &principal.sub, &claims)
        .await
        .map_err(|e| {
            OauthError::Provider(format!("failed to link ID-JAG subject to an account: {e}"))
        })?;

    let user = state
        .user_provider()
        .find_by_id(&user_id)
        .await
        .map_err(|e| OauthError::Provider(format!("failed to load the linked subject: {e}")))?
        .ok_or_else(|| OauthError::Provider("linked ID-JAG subject vanished".to_owned()))?;

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
