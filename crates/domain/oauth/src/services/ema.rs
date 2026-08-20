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

impl EnterprisePrincipal {
    fn verified_claims(&self) -> FederatedIdentityClaims {
        FederatedIdentityClaims {
            email_verified: self.email.is_some(),
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
