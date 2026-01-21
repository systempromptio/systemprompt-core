use super::{AnonymousSessionInfo, SessionCreationService, MAX_SESSION_AGE_SECONDS};
use crate::services::generation::{generate_anonymous_jwt, JwtSigningParams};
use systemprompt_analytics::MAX_SESSIONS_PER_FINGERPRINT;
use systemprompt_identifiers::{ClientId, SessionId, UserId};

const SESSION_LOOKUP_TIMEOUT_MS: u64 = 500;

impl SessionCreationService {
    pub(super) async fn try_reuse_session_at_limit(
        &self,
        fingerprint: &str,
        client_id: &ClientId,
        jwt_secret: &str,
    ) -> Option<AnonymousSessionInfo> {
        let fp_repo = self.fingerprint_repo.as_ref()?;

        let active_count = fp_repo
            .count_active_sessions(fingerprint)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, fingerprint = %fingerprint, "Failed to count active sessions");
                e
            })
            .ok()?;
        if active_count < MAX_SESSIONS_PER_FINGERPRINT {
            return None;
        }

        let session_id_str = fp_repo
            .find_reusable_session(fingerprint)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, fingerprint = %fingerprint, "Failed to find reusable session");
                e
            })
            .ok()
            .flatten()?;

        let existing_session = self
            .analytics_service
            .find_recent_session_by_fingerprint(fingerprint, MAX_SESSION_AGE_SECONDS)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, fingerprint = %fingerprint, "Failed to find recent session");
                e
            })
            .ok()
            .flatten()?;

        let user_id_str = existing_session.user_id.as_ref()?;
        let user_id = UserId::new(user_id_str.clone());
        let session_id = SessionId::new(session_id_str);

        let config = systemprompt_models::Config::get()
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to get config for session reuse");
                e
            })
            .ok()?;
        let signing = JwtSigningParams {
            secret: jwt_secret,
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(user_id_str, session_id.as_str(), client_id, &signing)
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to generate JWT for session reuse");
                e
            })
            .ok()?;

        tracing::debug!(
            fingerprint = %fingerprint,
            session_id = %session_id,
            active_sessions = active_count,
            "Reusing session due to fingerprint session limit"
        );

        Some(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: false,
            jwt_token: token,
        })
    }

    pub(super) async fn try_find_existing_session(
        &self,
        fingerprint: &str,
        client_id: &ClientId,
        jwt_secret: &str,
    ) -> Option<AnonymousSessionInfo> {
        let lookup_result = tokio::time::timeout(
            tokio::time::Duration::from_millis(SESSION_LOOKUP_TIMEOUT_MS),
            self.analytics_service
                .find_recent_session_by_fingerprint(fingerprint, MAX_SESSION_AGE_SECONDS),
        )
        .await;

        let existing_session = lookup_result
            .map_err(|_| {
                tracing::debug!(fingerprint = %fingerprint, "Session lookup timed out");
            })
            .ok()?
            .map_err(|e| {
                tracing::warn!(error = %e, fingerprint = %fingerprint, "Failed to find existing session");
                e
            })
            .ok()?
            .flatten()?;
        let user_id_str = existing_session.user_id.as_ref()?;

        let user_id = UserId::new(user_id_str.clone());
        let session_id = SessionId::new(existing_session.session_id.clone());

        let config = systemprompt_models::Config::get()
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to get config for session lookup");
                e
            })
            .ok()?;
        let signing = JwtSigningParams {
            secret: jwt_secret,
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(
            user_id_str,
            &existing_session.session_id,
            client_id,
            &signing,
        )
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
        })
    }
}
