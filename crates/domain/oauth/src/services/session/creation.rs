//! Anonymous session creation: mints a new user, session row, and signed JWT.

use super::{AnonymousSessionInfo, SessionCreationParams, SessionCreationService};
use crate::error::{OauthError, OauthResult};
use crate::services::generation::{JwtSigningParams, generate_anonymous_jwt};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::{CreateSessionInput, UserEvent};
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
            .map_err(|e| OauthError::Session(e.to_string()))?;
        let user_id = UserId::new(anonymous_user.id);

        let jwt_expiration_seconds = systemprompt_models::Config::get()
            .map_err(|e| OauthError::Config(e.to_string()))?
            .jwt_access_token_expiration;
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_expiration_seconds);

        self.analytics_provider
            .create_session(CreateSessionInput {
                session_id: &session_id,
                user_id: Some(&user_id),
                analytics: &params.analytics,
                session_source: params.session_source,
                is_bot: params.is_bot,
                is_ai_crawler: params.is_ai_crawler,
                expires_at,
            })
            .await
            .map_err(|e| OauthError::Session(e.to_string()))?;

        let config =
            systemprompt_models::Config::get().map_err(|e| OauthError::Config(e.to_string()))?;
        let signing = JwtSigningParams {
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(&user_id, &session_id, params.client_id, &signing)
            .map_err(|e| OauthError::TokenInvalid(e.to_string()))?;

        self.publish_event(UserEvent::UserCreated {
            user_id: user_id.clone(),
        });
        self.publish_event(UserEvent::SessionCreated {
            user_id: user_id.clone(),
            session_id: session_id.clone(),
        });

        Ok(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: true,
            jwt_token: token,
            fingerprint_hash: params.fingerprint.clone(),
        })
    }
}
