use std::sync::Arc;

use anyhow::Result;
use axum::http::HeaderMap;
use systemprompt_core_oauth::{
    validate_jwt_token, CreateAnonymousSessionInput, SessionCreationService,
};
use systemprompt_core_users::{UserProviderImpl, UserService};
use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_runtime::AppContext;

use crate::services::middleware::jwt::extract_token_from_headers;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub is_new: bool,
    pub jwt_token: Option<String>,
}

pub async fn ensure_session(
    headers: &HeaderMap,
    uri: Option<&http::Uri>,
    ctx: &AppContext,
) -> Result<SessionInfo> {
    let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
    let config = systemprompt_models::Config::get()?;

    if let Some(token) = extract_token_from_headers(headers) {
        if let Ok(claims) = validate_jwt_token(
            &token,
            jwt_secret,
            &config.jwt_issuer,
            &config.jwt_audiences,
        ) {
            if let Some(session_id) = claims.session_id {
                return Ok(SessionInfo {
                    session_id: SessionId::new(session_id),
                    user_id: UserId::new(claims.sub),
                    is_new: false,
                    jwt_token: Some(token),
                });
            }
        }
    }

    let user_service = UserService::new(ctx.db_pool())?;
    let session_service = SessionCreationService::new(
        ctx.analytics_service().clone(),
        Arc::new(UserProviderImpl::new(user_service)),
    );

    let client_id = ClientId::new("sp_web".to_string());
    let session_info = session_service
        .create_anonymous_session(CreateAnonymousSessionInput {
            headers,
            uri,
            client_id: &client_id,
            jwt_secret,
            session_source: SessionSource::Web,
        })
        .await?;

    Ok(SessionInfo {
        session_id: session_info.session_id,
        user_id: session_info.user_id,
        is_new: session_info.is_new,
        jwt_token: Some(session_info.jwt_token),
    })
}
