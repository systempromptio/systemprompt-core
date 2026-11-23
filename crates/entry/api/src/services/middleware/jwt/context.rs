//! [`JwtContextExtractor`]: the authenticator's audiences for a mount, and
//! the A2A body's context binding.
//!
//! The extractor admits the first-party audiences. Only the one
//! [`JwtContextExtractor::admitting_ui_sessions`] returns, which the documents,
//! folders and uploads routes and OAuth `/revoke` mount, also admits the `ui`
//! audience of a person's UI session. On A2A routes the context id comes from
//! the JSON-RPC body, and an `X-Context-ID` header without a bearer is
//! refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;

use super::SessionClaims;
use crate::services::middleware::context::ContextExtractor;
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::{ContextId, ExtensionId, JwtToken, UserId, headers as sp_headers};
use systemprompt_models::auth::{DynRequestAuthenticator, JwtAudience, JwtUserContext};
use systemprompt_models::execution::context::{ContextExtractionError, RequestContext};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_traits::AuthUser;

#[derive(Clone, Debug)]
pub struct JwtContextExtractor {
    authenticator: DynRequestAuthenticator,
    claims: SessionClaims,
    audiences: &'static [JwtAudience],
}

impl JwtContextExtractor {
    pub fn from_kernel(state: &Dependencies) -> Result<Self, LoaderError> {
        let authenticator =
            state
                .get::<DynRequestAuthenticator>()
                .cloned()
                .map_err(|_missing| {
                    LoaderError::missing_provider::<DynRequestAuthenticator>(ExtensionId::new(
                        "users",
                    ))
                })?;
        Ok(Self {
            authenticator,
            claims: SessionClaims::from_kernel(state)?,
            audiences: JwtAudience::FIRST_PARTY,
        })
    }

    #[must_use]
    pub const fn admitting_ui_sessions(mut self) -> Self {
        self.audiences = JwtAudience::FIRST_PARTY_AND_UI;
        self
    }

    #[must_use]
    pub const fn authenticator(&self) -> &DynRequestAuthenticator {
        &self.authenticator
    }

    pub async fn extract_standard(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.authenticator
            .authenticate(headers, self.audiences)
            .await
    }

    pub async fn load_user(
        &self,
        user_id: &UserId,
        route_context: &str,
    ) -> Result<AuthUser, ContextExtractionError> {
        self.authenticator.load_user(user_id, route_context).await
    }

    pub(crate) fn verified_claims(&self, token: &str) -> Option<JwtUserContext> {
        self.claims.verified_claims(token)
    }

    pub async fn decode_for_audiences(
        &self,
        jwt_token: &JwtToken,
        audiences: &[JwtAudience],
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError> {
        self.authenticator
            .decode_for_audiences(jwt_token, audiences)
            .await
    }

    pub async fn decode_for_gateway(
        &self,
        jwt_token: &JwtToken,
    ) -> Result<(JwtUserContext, AuthUser), ContextExtractionError> {
        self.authenticator.decode_for_gateway(jwt_token).await
    }

    async fn extract_from_request_impl(
        &self,
        request: Request<Body>,
        body_limit: usize,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        use crate::services::middleware::context::sources::{ContextIdSource, PayloadSource};

        let headers = request.headers().clone();
        let has_auth = headers.get("authorization").is_some();

        if headers.get(sp_headers::CONTEXT_ID).is_some() && !has_auth {
            return Err(ContextExtractionError::ForbiddenHeader {
                header: "X-Context-ID".to_owned(),
                reason: "Context ID must be in request body (A2A spec). Use contextId field in \
                         message."
                    .to_owned(),
            });
        }

        let ctx = self
            .authenticator
            .authenticate(&headers, JwtAudience::FIRST_PARTY)
            .await?;

        let (body_bytes, reconstructed_request) =
            PayloadSource::read_and_reconstruct(request, body_limit).await?;

        let ctx = match PayloadSource::extract_context_source(&body_bytes)? {
            ContextIdSource::Direct(id) => {
                ctx.with_context_id(ContextId::try_new(id).map_err(|_invalid_id| {
                    ContextExtractionError::InvalidHeaderValue {
                        header: "contextId".to_owned(),
                        reason: "not a valid context id".to_owned(),
                    }
                })?)
            },
            ContextIdSource::FromTask { task_id } => ctx
                .with_context_id(ContextId::derived_from_task(&task_id))
                .with_task_id(task_id),
        };
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
        body_limit: usize,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        self.extract_from_request_impl(request, body_limit).await
    }
}
