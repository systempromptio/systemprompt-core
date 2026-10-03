//! OAuth session binding for minted tokens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_oauth::OAuthState;
use systemprompt_traits::{AnalyticsProviderError, CreateSessionInput, ExtractSignals};

use crate::RequestOrigin;

pub(crate) async fn create_oauth_session(
    state: &OAuthState,
    origin: RequestOrigin<'_>,
    user_id: &UserId,
    expires_in: i64,
) -> Result<SessionId, AnalyticsProviderError> {
    let session_id = SessionId::new(format!("sess_{}", uuid::Uuid::new_v4().simple()));
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);
    let analytics = state.analytics_provider().extract_analytics(
        origin.headers,
        ExtractSignals {
            caller_ip: origin.caller_ip,
            ..Default::default()
        },
    );
    state
        .session_provider()
        .create_session(CreateSessionInput {
            session_id: &session_id,
            user_id: Some(user_id),
            analytics: &analytics,
            session_source: SessionSource::Oauth,
            is_bot: false,
            is_ai_crawler: false,
            expires_at,
        })
        .await?;
    Ok(session_id)
}
