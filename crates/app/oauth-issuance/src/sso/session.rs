//! The browser session every SSO sign-in ends in: an authenticated session
//! row, a signed access token, and how long the cookie carrying it may live.
//!
//! A token is checked on `permissions`, not roles, so a subject holding the
//! `admin` role is minted `Admin` in addition to (never instead of) `User`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AccessTokenId, SessionId, SessionSource, UserId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{AuthenticatedUser, Permission, UserJwtRequest};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_traits::SessionAnalytics;
use systemprompt_traits::sso::SsoError;

const ADMIN_ROLE: &str = "admin";

/// Who the session is for.
#[derive(Debug, Clone, Copy)]
pub struct BrowserSessionSubject<'a> {
    pub user_id: &'a UserId,
    pub display_name: &'a str,
    pub email: &'a str,
    pub roles: &'a [String],
}

#[derive(Debug, Clone)]
pub struct MintedSession {
    pub access_token: String,
    pub max_age_secs: i64,
    pub session_id: SessionId,
}

#[must_use]
pub fn permissions_for_roles(roles: &[String]) -> Vec<Permission> {
    if roles.iter().any(|role| role == ADMIN_ROLE) {
        vec![Permission::Admin, Permission::User]
    } else {
        vec![Permission::User]
    }
}

pub async fn mint_browser_session(
    session_service: &SessionCreationService,
    config: &Config,
    subject: &BrowserSessionSubject<'_>,
    analytics: &SessionAnalytics,
) -> Result<MintedSession, SsoError> {
    let session_id = session_service
        .create_authenticated_session(subject.user_id, analytics, SessionSource::Oauth)
        .await
        .map_err(SsoError::internal)?;

    let permissions = permissions_for_roles(subject.roles);
    let user = AuthenticatedUser::new_with_roles(
        subject.user_id.clone(),
        subject.display_name.to_owned(),
        subject.email.to_owned(),
        permissions.clone(),
        subject.roles.to_vec(),
    );
    let request = UserJwtRequest {
        user,
        session_id: session_id.clone(),
        jti: AccessTokenId::generate(),
        permissions,
        audience: config.jwt_audiences.clone(),
        ttl: chrono::Duration::seconds(config.jwt_access_token_expiration),
        client_id: None,
        purpose: None,
        plugin_id: None,
        interaction: None,
    };
    let access_token = session_service
        .token_authority()
        .mint_user_jwt(&request)
        .map_err(SsoError::internal)?;

    Ok(MintedSession {
        access_token,
        max_age_secs: config.jwt_access_token_expiration,
        session_id,
    })
}
