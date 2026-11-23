//! The users member's composed provides: the sealed gateway session mint
//! conferred on `gateway-http`, the sealed sign-in session view conferred on
//! `identity-http`, and the open request authenticator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_manifest::Config;
use systemprompt_models::auth::{DynRequestAuthenticator, DynTokenRevocations};
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_security::{
    JwtRequestAuthenticator, JwtRequestAuthenticatorParts, UnavailableRevocations,
};
use systemprompt_traits::{DynGatewaySessionMint, DynSignInSessions};

use crate::repository::{SessionRepository, UserRepository};
use crate::{ApiKeyLifetime, UserService, UsersGatewaySessionMint};

pub(crate) fn gateway_session_mint(
    r: &Resolved<'_>,
) -> Result<DynGatewaySessionMint, ProvideError> {
    let db = r.get::<DbPool>()?;
    let config = r.get::<Arc<Config>>()?;
    Ok(Arc::new(UsersGatewaySessionMint::new(
        Arc::new(UserRepository::new(&db)),
        SessionRepository::new(&db),
        ApiKeyLifetime::from_max_days(config.api_key_max_lifetime_days),
        chrono::Duration::seconds(config.jwt_access_token_expiration),
    )))
}

pub(crate) fn sign_in_sessions(r: &Resolved<'_>) -> Result<DynSignInSessions, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(SessionRepository::new(&db)))
}

pub(crate) fn request_authenticator(
    r: &Resolved<'_>,
) -> Result<DynRequestAuthenticator, ProvideError> {
    let db = r.get::<DbPool>()?;
    let config = r.get::<Arc<Config>>()?;
    let revocations = r
        .optional::<DynTokenRevocations>()
        .unwrap_or_else(|| Arc::new(UnavailableRevocations));
    Ok(Arc::new(JwtRequestAuthenticator::new(
        JwtRequestAuthenticatorParts {
            sessions: Arc::new(SessionRepository::new(&db)),
            users: Arc::new(UserService::new(Arc::new(UserRepository::new(&db)))),
            revocations,
            issuer: config.jwt_issuer.clone(),
            keyring: r.get::<VerificationKeyring>()?,
        },
    )))
}
