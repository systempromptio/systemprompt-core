//! Builds the one [`JwtContextExtractor`] configuration from the app context:
//! its session and user providers, the jti revocation checker over the OAuth
//! repository, the configured issuer and the signing keys. Both the server's
//! context middleware and the gateway's handlers use it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, SessionProvider, UserProvider};

use super::{JtiRevocationChecker, JwtContextExtractor};

impl JwtContextExtractor {
    pub fn from_app_context(ctx: &AppContext) -> Result<Self, LoaderError> {
        let sessions = ctx
            .session_provider()
            .ok_or_else(missing::<Arc<dyn SessionProvider>>)?;
        let users = ctx
            .user_provider()
            .ok_or_else(missing::<Arc<dyn UserProvider>>)?;
        let jti_revocation = JtiRevocationChecker::from_repository(
            ctx.oauth_repositories().oauth.clone(),
            ctx.cache_epochs().clone(),
        );
        Ok(Self::new(
            sessions,
            users,
            jti_revocation,
            ctx.config().jwt_issuer.clone(),
            ctx.signing_keys().clone(),
        ))
    }
}

fn missing<T: std::any::Any>() -> LoaderError {
    LoaderError::missing_provider::<T>(ExtensionId::new("jwt"))
}
