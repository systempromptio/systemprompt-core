//! Anonymous session creation: mints a new user, session row, and signed JWT.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AnonymousSessionInfo, SessionCreationParams, SessionCreationService};
use crate::error::{OauthError, OauthResult};
use crate::services::generation::generate_anonymous_jwt_with_expiry;
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_traits::CreateSessionInput;
use uuid::Uuid;

impl SessionCreationService {
    pub(super) async fn create_new_session(
        &self,
        params: SessionCreationParams<'_>,
    ) -> OauthResult<AnonymousSessionInfo> {
        let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));

        let anonymous_user = self
            .user_provider
            .create_anonymous(&params.fingerprint)
            .await
            .map_err(|source| OauthError::UserProvider {
                context: "creating the anonymous user",
                source,
            })?;
        let user_id = UserId::new(anonymous_user.id);

        let jwt_expiration_seconds = self.config.jwt_access_token_expiration;
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_expiration_seconds);

        self.session_provider
            .create_session(CreateSessionInput {
                session_id: &session_id,
                user_id: Some(&user_id),
                analytics: params.analytics,
                session_source: params.session_source,
                is_bot: params.analytics.is_bot,
                is_ai_crawler: params.analytics.is_ai_crawler,
                expires_at,
            })
            .await?;

        let token = generate_anonymous_jwt_with_expiry(
            self.authority.as_ref(),
            &user_id,
            &session_id,
            params.client_id,
            jwt_expiration_seconds,
        )?;

        Ok(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: true,
            jwt_token: JwtToken::new(token),
            fingerprint_hash: params.fingerprint.clone(),
        })
    }
}
