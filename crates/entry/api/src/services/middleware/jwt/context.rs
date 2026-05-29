//! JWT-backed request-context extractor.
//!
//! [`JwtContextExtractor`] implements [`ContextExtractor`] by validating the
//! bearer token (signature, session existence, user existence, and JTI
//! revocation) and building a `RequestContext`. It resolves the context id from
//! the `x-context-id` header on standard routes and from the JSON-RPC body on
//! A2A routes, and exposes a gateway decode path for pre-authenticated tokens.

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use std::sync::Arc;

use crate::services::middleware::context::ContextExtractor;
use systemprompt_identifiers::ContextId;
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};
use systemprompt_security::{JwtUserContext, TokenExtractor, extract_user_context};
use systemprompt_traits::{AnalyticsProvider, UserProvider};

use super::params::{BuildContextParams, build_context, extract_common_headers};
use super::revocation::JtiRevocationChecker;
use super::validation::{UserCache, user_is_admin, validate_session_exists, validate_user_exists};

#[derive(Clone)]
pub struct JwtContextExtractor {
    token_extractor: TokenExtractor,
    analytics_provider: Arc<dyn AnalyticsProvider>,
    user_provider: Arc<dyn UserProvider>,
    user_cache: Arc<UserCache>,
    jti_revocation: JtiRevocationChecker,
}

impl std::fmt::Debug for JwtContextExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtContextExtractor")
            .field("token_extractor", &self.token_extractor)
            .finish_non_exhaustive()
    }
}

impl JwtContextExtractor {
    pub fn new(
        analytics_provider: Arc<dyn AnalyticsProvider>,
        user_provider: Arc<dyn UserProvider>,
        jti_revocation: JtiRevocationChecker,
    ) -> Self {
        Self {
            token_extractor: TokenExtractor::browser_only(),
            analytics_provider,
            user_provider,
            user_cache: UserCache::new(),
            jti_revocation,
        }
    }

    fn extract_jwt_context(
        &self,
        headers: &HeaderMap,
    ) -> Result<JwtUserContext, ContextExtractionError> {
        let token = self
            .token_extractor
            .extract(headers)
            .map_err(|_e| ContextExtractionError::MissingAuthHeader)?;
        extract_user_context(&token)
            .map_err(|e| ContextExtractionError::InvalidToken(e.to_string()))
    }

    async fn validate(
        &self,
        jwt_context: &JwtUserContext,
        route_context: &str,
    ) -> Result<systemprompt_traits::AuthUser, ContextExtractionError> {
        if jwt_context.session_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingSessionId);
        }
        if jwt_context.user_id.as_str().is_empty() {
            return Err(ContextExtractionError::MissingUserId);
        }
        let validated = validate_user_exists(
            &self.user_provider,
            &self.user_cache,
            jwt_context,
            route_context,
        )
        .await?;
        validate_session_exists(&self.analytics_provider, jwt_context, route_context).await?;
        self.jti_revocation
            .ensure_not_revoked(&jwt_context.jti)
            .await?;
        Ok(validated.user)
    }

    pub async fn extract_standard(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        let jwt_context = self.extract_jwt_context(headers)?;
        let user = self.validate(&jwt_context, "").await?;

        let context_id = headers
            .get("x-context-id")
            .and_then(|h| h.to_str().ok())
            .filter(|s| !s.is_empty())
            .and_then(|s| ContextId::try_new(s).ok())
            .unwrap_or_else(ContextId::generate);

        let (trace_id, task_id, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, headers);

        let user_type = jwt_context.user_type.reconcile_with(user_is_admin(&user));
        let session_id = jwt_context.session_id.clone();
        let user_id = jwt_context.user_id.clone();

        Ok(build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
            user_type,
        }))
    }

    pub async fn decode_for_gateway(
        &self,
        jwt_token: &systemprompt_identifiers::JwtToken,
    ) -> Result<(JwtUserContext, systemprompt_traits::AuthUser), ContextExtractionError> {
        let jwt_context = extract_user_context(jwt_token.as_str())
            .map_err(|e| ContextExtractionError::InvalidToken(e.to_string()))?;

        let user = self.validate(&jwt_context, "gateway").await?;
        Ok((jwt_context, user))
    }

    async fn extract_from_request_impl(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        use crate::services::middleware::context::sources::{ContextIdSource, PayloadSource};

        let headers = request.headers().clone();
        let has_auth = headers.get("authorization").is_some();

        if headers.get("x-context-id").is_some() && !has_auth {
            return Err(ContextExtractionError::ForbiddenHeader {
                header: "X-Context-ID".to_owned(),
                reason: "Context ID must be in request body (A2A spec). Use contextId field in \
                         message."
                    .to_owned(),
            });
        }

        let jwt_context = self.extract_jwt_context(&headers)?;
        let user = self.validate(&jwt_context, " (A2A route)").await?;

        let (body_bytes, reconstructed_request) =
            PayloadSource::read_and_reconstruct(request).await?;

        let context_source = PayloadSource::extract_context_source(&body_bytes)?;
        let (context_id, task_id_from_payload) = match context_source {
            ContextIdSource::Direct(id) => (
                ContextId::try_new(id).map_err(|e| ContextExtractionError::InvalidHeaderValue {
                    header: "contextId".to_owned(),
                    reason: e.to_string(),
                })?,
                None,
            ),
            ContextIdSource::FromTask { task_id } => (ContextId::generate(), Some(task_id)),
        };

        let (trace_id, task_id_from_header, auth_token, agent_name) =
            extract_common_headers(&self.token_extractor, &headers);

        let task_id = task_id_from_payload.or(task_id_from_header);
        let user_type = jwt_context.user_type.reconcile_with(user_is_admin(&user));

        let session_id = jwt_context.session_id.clone();
        let user_id = jwt_context.user_id.clone();
        let ctx = build_context(BuildContextParams {
            jwt_context,
            session_id,
            user_id,
            trace_id,
            context_id,
            agent_name,
            task_id,
            auth_token,
            user_type,
        });

        Ok((ctx, reconstructed_request))
    }
}

#[async_trait]
impl ContextExtractor for JwtContextExtractor {
    async fn extract_from_headers(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.extract_standard(headers).await
    }

    async fn extract_from_request(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        self.extract_from_request_impl(request).await
    }
}
