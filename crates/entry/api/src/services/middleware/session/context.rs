//! Establishing the per-request [`RequestContext`].
//!
//! Three ways a request acquires one, in descending order of fidelity: a
//! tracked session resolved or minted for a real visitor, an anonymous
//! context for a path we have decided not to track, and a degraded context
//! for a request whose session could not be established at all.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_analytics::AnalyticsService;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_security::{HeaderExtractor, TokenExtractor, extract_user_context};
use systemprompt_traits::AnalyticsProvider;
use uuid::Uuid;

use super::{RequestMeta, SessionMiddleware, attest_session, lifecycle};

// Why: the pool's own acquire timeout is 30s, which is a page load nobody
// waits for and, per connection a browser opens, a site that reads as hung
// rather than degraded. A healthy anonymous-user lookup is single-digit
// milliseconds, so this leaves a hundredfold margin before we give up on it.
const SESSION_ESTABLISH_TIMEOUT: Duration = Duration::from_secs(2);

impl SessionMiddleware {
    pub(super) async fn establish_or_degrade(
        &self,
        should_skip: bool,
        trace_id: systemprompt_identifiers::TraceId,
        meta: &RequestMeta<'_>,
        path: &str,
    ) -> (RequestContext, Option<String>) {
        let establish = async {
            if should_skip {
                Ok((
                    self.anonymous_context("untracked", trace_id.clone(), meta)
                        .await?,
                    None,
                ))
            } else {
                self.tracked_context(trace_id.clone(), meta).await
            }
        };

        match tokio::time::timeout(SESSION_ESTABLISH_TIMEOUT, establish).await {
            Ok(Ok(established)) => established,
            Ok(Err(e)) => {
                self.warn_degraded(path, &e.message);
                (Self::degraded_context(trace_id), None)
            },
            Err(_) => {
                self.warn_degraded(
                    path,
                    "timed out establishing a session; the database did not answer",
                );
                (Self::degraded_context(trace_id), None)
            },
        }
    }

    fn warn_degraded(&self, path: &str, reason: &str) {
        if self.degraded_warn.allow() {
            tracing::warn!(
                path,
                reason,
                "serving without a session; page views are not being attributed until the \
                 database recovers"
            );
        }
    }

    // Why: no actor and no auth token, so the `unset` user this leaves in
    // place cannot be mistaken for an identity and every gate above `public`
    // still refuses the request. `is_tracked` is false, which is what keeps
    // the analytics sinks from recording a visit they cannot attribute.
    fn degraded_context(trace_id: systemprompt_identifiers::TraceId) -> RequestContext {
        let session_id = SessionId::new(format!("degraded_{}", Uuid::new_v4()));
        let context_id = ContextId::derived_from_session(&session_id);
        RequestContext::new(session_id, trace_id, context_id, AgentName::system())
            .with_user_type(UserType::Anon)
            .with_tracked(false)
    }

    async fn anonymous_context(
        &self,
        session_prefix: &str,
        trace_id: systemprompt_identifiers::TraceId,
        meta: &RequestMeta<'_>,
    ) -> Result<RequestContext, ApiError> {
        let (user_id, fingerprint) = self
            .session_creation_service
            .ensure_anonymous_user(meta.analytics)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, session_prefix, "Failed to ensure anonymous user");
                ApiError::internal_error("Service temporarily unavailable")
            })?;

        let session_id = SessionId::new(format!("{session_prefix}_{}", Uuid::new_v4()));
        let context_id = ContextId::derived_from_session(&session_id);
        Ok(
            RequestContext::new(session_id, trace_id, context_id, AgentName::system())
                .with_actor(systemprompt_identifiers::Actor::anonymous(user_id))
                .with_user_type(UserType::Anon)
                .with_tracked(false)
                .with_fingerprint_hash(fingerprint),
        )
    }

    async fn tracked_context(
        &self,
        trace_id: systemprompt_identifiers::TraceId,
        meta: &RequestMeta<'_>,
    ) -> Result<(RequestContext, Option<String>), ApiError> {
        tracing::debug!(
            path = %meta.uri.path(),
            skip_tracking = meta.analytics.skip_tracking,
            user_agent = ?meta.analytics.user_agent,
            "Session middleware bot check"
        );

        if meta.analytics.skip_tracking {
            return Ok((self.anonymous_context("bot", trace_id, meta).await?, None));
        }

        let token_result = TokenExtractor::browser_only().extract(meta.headers).ok();

        let (session_id, user_id, jwt_token, jwt_cookie, fingerprint_hash) =
            self.resolve_session(token_result, meta).await?;

        let context_id = HeaderExtractor::extract_context_id(meta.headers)
            .unwrap_or_else(|| ContextId::derived_from_session(&session_id));

        let mut ctx = RequestContext::new(session_id, trace_id, context_id, AgentName::system())
            .with_actor(systemprompt_identifiers::Actor::user(user_id))
            .with_auth_token(jwt_token)
            .with_user_type(UserType::Anon)
            .with_tracked(true);
        if let Some(fp) = fingerprint_hash {
            ctx = ctx.with_fingerprint_hash(fp);
        }
        Ok((ctx, jwt_cookie))
    }

    async fn resolve_session(
        &self,
        token_result: Option<String>,
        meta: &RequestMeta<'_>,
    ) -> Result<(SessionId, UserId, String, Option<String>, Option<String>), ApiError> {
        let Some(token) = token_result else {
            let (sid, uid, token, is_new, fp) =
                lifecycle::create_new_session(&self.session_creation_service, meta).await?;
            let jwt_cookie = if is_new { Some(token.clone()) } else { None };
            return Ok((sid, uid, token, jwt_cookie, Some(fp)));
        };

        let Ok(jwt_context) = extract_user_context(&token) else {
            let (sid, uid, token, is_new, fp) =
                lifecycle::create_new_session(&self.session_creation_service, meta).await?;
            let jwt_cookie = if is_new { Some(token.clone()) } else { None };
            return Ok((sid, uid, token, jwt_cookie, Some(fp)));
        };

        let analytics_provider: Arc<dyn AnalyticsProvider> =
            Arc::<AnalyticsService>::clone(&self.analytics_service);

        match attest_session(
            &analytics_provider,
            &jwt_context.session_id,
            &jwt_context.user_id,
            "session_middleware",
        )
        .await
        {
            Ok(()) => {
                return Ok((
                    jwt_context.session_id,
                    jwt_context.user_id,
                    token,
                    None,
                    None,
                ));
            },
            Err(e) => tracing::info!(
                old_session_id = %jwt_context.session_id,
                user_id = %jwt_context.user_id,
                reason = %e,
                "JWT session failed attestation, refreshing with new session"
            ),
        }

        match lifecycle::refresh_session_for_user(
            &self.session_creation_service,
            &jwt_context.user_id,
            meta,
        )
        .await
        {
            Ok((sid, uid, new_token, _, fp)) => {
                Ok((sid, uid, new_token.clone(), Some(new_token), Some(fp)))
            },
            Err(e) if e.error_key.as_deref() == Some("user_not_found") => {
                tracing::warn!(
                    user_id = %jwt_context.user_id,
                    "JWT references non-existent user, creating new anonymous session"
                );
                let (sid, uid, token, _, fp) =
                    lifecycle::create_new_session(&self.session_creation_service, meta).await?;
                Ok((sid, uid, token.clone(), Some(token), Some(fp)))
            },
            Err(e) => Err(e),
        }
    }
}
