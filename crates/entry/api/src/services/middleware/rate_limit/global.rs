//! The database-backed window keyed by verified identity, which bounds a
//! caller's budget across every replica of the deployment.
//!
//! It applies only to routers that carry `with_auth`: the identity comes from
//! a [`RequestContext`] in the request's extensions, and a router that
//! authenticates inside its handlers never puts one there. On those routers
//! every request is anonymous to this layer and passes straight through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_users::UserRateLimitBucketRepository;

pub(super) const GLOBAL_WINDOW_SECS: i64 = 10;

#[derive(Clone, Debug)]
pub(super) struct GlobalUserLimit {
    pub(super) buckets: Arc<UserRateLimitBucketRepository>,
    pub(super) scope: &'static str,
    pub(super) budget: i64,
}

fn window_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let secs = now.timestamp();
    let start = secs - secs.rem_euclid(GLOBAL_WINDOW_SECS);
    DateTime::from_timestamp(start, 0).unwrap_or(now)
}

fn too_many_requests(now: DateTime<Utc>, start: DateTime<Utc>) -> Response {
    let elapsed = now.timestamp() - start.timestamp();
    let retry_after = (GLOBAL_WINDOW_SECS - elapsed).max(1);
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, retry_after.to_string())],
        "rate limit exceeded",
    )
        .into_response()
}

pub(super) async fn global_user_rate_limit(
    State(limit): State<GlobalUserLimit>,
    req: Request,
    next: Next,
) -> Response {
    let user_id = req
        .extensions()
        .get::<RequestContext>()
        .filter(|ctx| ctx.auth.user_type != UserType::Anon)
        .map(|ctx| ctx.user_id().clone());
    let Some(user_id) = user_id else {
        return next.run(req).await;
    };

    let now = Utc::now();
    let start = window_start(now);
    match limit.buckets.hit(&user_id, limit.scope, start).await {
        Ok(hits) if hits > limit.budget => {
            tracing::debug!(
                user_id = %user_id,
                scope = limit.scope,
                hits,
                budget = limit.budget,
                "global user rate limit exceeded"
            );
            too_many_requests(now, start)
        },
        Ok(_) => next.run(req).await,
        Err(err) => {
            tracing::warn!(
                user_id = %user_id,
                scope = limit.scope,
                error = %err,
                "global user rate limit unavailable; admitting request"
            );
            next.run(req).await
        },
    }
}
