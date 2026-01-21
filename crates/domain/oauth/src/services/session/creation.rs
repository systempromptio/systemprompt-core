use super::{AnonymousSessionInfo, SessionCreationParams, SessionCreationService};
use crate::services::generation::{generate_anonymous_jwt, JwtSigningParams};
use anyhow::Result;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::UserEvent;
use uuid::Uuid;

impl SessionCreationService {
    pub(super) async fn create_new_session(
        &self,
        params: SessionCreationParams<'_>,
    ) -> Result<AnonymousSessionInfo> {
        let session_id = SessionId::new(format!("sess_{}", Uuid::new_v4()));

        let anonymous_user = self
            .user_provider
            .create_anonymous(&params.fingerprint)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        let user_id = UserId::new(anonymous_user.id);

        let jwt_expiration_seconds =
            systemprompt_models::Config::get()?.jwt_access_token_expiration;
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_expiration_seconds);

        self.analytics_service
            .create_analytics_session(systemprompt_analytics::CreateAnalyticsSessionInput {
                session_id: &session_id,
                user_id: Some(&user_id),
                analytics: &params.analytics,
                session_source: params.session_source,
                is_bot: params.is_bot,
                expires_at,
            })
            .await?;

        let config = systemprompt_models::Config::get()?;
        let signing = JwtSigningParams {
            secret: params.jwt_secret,
            issuer: &config.jwt_issuer,
        };
        let token = generate_anonymous_jwt(
            user_id.as_str(),
            session_id.as_str(),
            params.client_id,
            &signing,
        )?;

        self.publish_event(UserEvent::UserCreated {
            user_id: user_id.to_string(),
        });
        self.publish_event(UserEvent::SessionCreated {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        });

        Ok(AnonymousSessionInfo {
            session_id,
            user_id,
            is_new: true,
            jwt_token: token,
        })
    }
}
