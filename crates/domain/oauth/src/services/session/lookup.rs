//! Existing anonymous-session lookup and JWT regeneration.
//!
//! A fingerprint is attacker-reproducible, so a session it finds is reused only
//! when its owner is still an anonymous user, and the minted token always pairs
//! the session id with the user id of that same session row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AnonymousSessionInfo, MAX_SESSION_AGE_SECONDS, SessionCreationService};
use crate::services::generation::{JwtSigningParams, generate_anonymous_jwt};
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::auth::UserRole;

const SESSION_LOOKUP_TIMEOUT_MS: u64 = 500;

impl SessionCreationService {
    pub(super) async fn try_find_existing_session(
        &self,
        fingerprint: &str,
        client_id: &ClientId,
    ) -> Option<AnonymousSessionInfo> {
        let lookup_result = tokio::time::timeout(
            tokio::time::Duration::from_millis(SESSION_LOOKUP_TIMEOUT_MS),
            self.session_provider
                .find_recent_session_by_fingerprint(fingerprint, MAX_SESSION_AGE_SECONDS),
        )
        .await;

        let existing_session = lookup_result
            .map_err(|_e| {
                tracing::debug!(fingerprint = %fingerprint, "Session lookup timed out");
            })
            .ok()?
            .map_err(|e| {
                tracing::warn!(error = %e, fingerprint = %fingerprint, "Failed to find existing session");
                e
            })
            .ok()??;
        let user_id_str = existing_session.user_id.as_ref()?;

        let user_id = UserId::new(user_id_str.clone());
        let session_id = SessionId::new(existing_session.session_id.clone());

        if !self.is_anonymous_user(&user_id).await {
            tracing::debug!(
                fingerprint = %fingerprint,
                "Fingerprint matched a non-anonymous session; not reusing it"
            );
            return None;
        }

        let config = systemprompt_models::Config::get()
            .inspect_err(|e| {
                tracing::warn!(error = %e, "Failed to get config for session lookup");
            })
            .ok()?;
        let signing = JwtSigningParams {
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(&user_id, &session_id, client_id, &signing)
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to generate JWT for session lookup");
                e
            })
            .ok()?;

        Some(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: false,
            jwt_token: token,
            fingerprint_hash: fingerprint.to_owned(),
        })
    }

    async fn is_anonymous_user(&self, user_id: &UserId) -> bool {
        let anonymous_role = UserRole::Anonymous.as_str();
        match self.user_provider.find_by_id(user_id).await {
            Ok(Some(user)) => user.roles.iter().any(|role| role == anonymous_role),
            Ok(None) => false,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load the owner of a fingerprint session");
                false
            },
        }
    }
}
