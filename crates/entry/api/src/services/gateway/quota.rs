//! Quota-window alignment and bucket accounting for gateway policies.
//!
//! Windows are keyed by a subject: the requesting user by default, or any
//! subject-attribute dimension an extension registers (for example
//! `organization`). Cost ceilings are enforced one request late — cost is
//! known only after the response.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use sqlx::PgPool;
use systemprompt_ai::USER_QUOTA_SUBJECT;
use systemprompt_ai::repository::{
    AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta, QuotaBucketState,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_security::authz::{
    AuthzHookContext, NullAuditSink, SharedSubjectAttributeProvider, discover_subject_providers,
};

use super::policy::QuotaWindow;

#[derive(Debug, Clone)]
pub struct QuotaDecision {
    pub allow: bool,
    pub window_seconds: i32,
    pub message: String,
    pub state: QuotaBucketState,
}

/// The resolved `(subject_kind, subject_id)` a window's bucket is keyed by.
struct WindowSubject<'a> {
    kind: &'a str,
    id: String,
}

fn subject_providers(pool: &Arc<PgPool>) -> &'static [SharedSubjectAttributeProvider] {
    static PROVIDERS: OnceLock<Vec<SharedSubjectAttributeProvider>> = OnceLock::new();
    PROVIDERS.get_or_init(|| {
        discover_subject_providers(&AuthzHookContext {
            pool: Arc::clone(pool),
            sink: Arc::new(NullAuditSink),
        })
    })
}

/// Resolve the bucket subject for a window. `user`-keyed windows resolve to
/// the requesting user; extension dimensions resolve through the registered
/// provider, first value winning. `None` (no provider or no value — e.g. a
/// user outside any organization) means the window does not apply.
async fn resolve_subject<'a>(
    window: &'a QuotaWindow,
    user_id: &UserId,
    pool: &Arc<PgPool>,
) -> Option<WindowSubject<'a>> {
    if window.subject == USER_QUOTA_SUBJECT {
        return Some(WindowSubject {
            kind: USER_QUOTA_SUBJECT,
            id: user_id.as_str().to_owned(),
        });
    }
    let provider = subject_providers(pool)
        .iter()
        .find(|p| p.dimension().rule_type.as_str() == window.subject)?;
    let id = provider.values_for(user_id).await.into_iter().next()?;
    Some(WindowSubject {
        kind: &window.subject,
        id,
    })
}

pub async fn precheck_and_reserve(
    db: &DbPool,
    user_id: &UserId,
    windows: &[QuotaWindow],
) -> Result<Option<QuotaDecision>> {
    if windows.is_empty() {
        return Ok(None);
    }
    let repo =
        AiQuotaBucketRepository::new(db).map_err(|e| anyhow::anyhow!("quota repo init: {e}"))?;
    let pool = db
        .pool_arc()
        .map_err(|e| anyhow::anyhow!("quota pool init: {e}"))?;

    let now = Utc::now();
    for window in windows {
        let Some(subject) = resolve_subject(window, user_id, &pool).await else {
            continue;
        };
        let window_start = align_window(now, window.window_seconds);
        let state = repo
            .increment(IncrementParams {
                subject_kind: subject.kind,
                subject_id: &subject.id,
                window_seconds: window.window_seconds,
                window_start,
                delta: QuotaBucketDelta {
                    requests: 1,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_microdollars: 0,
                },
            })
            .await?;

        if let Some(max) = window.max_requests
            && state.requests > max
        {
            return Ok(Some(QuotaDecision {
                allow: false,
                window_seconds: window.window_seconds,
                message: format!(
                    "quota exceeded for {} window {}s (used {}/{max})",
                    subject.kind, window.window_seconds, state.requests
                ),
                state,
            }));
        }

        if let Some(max) = window.max_cost_microdollars
            && state.cost_microdollars > max
        {
            return Ok(Some(QuotaDecision {
                allow: false,
                window_seconds: window.window_seconds,
                message: format!(
                    "cost ceiling exceeded for {} window {}s (spent {}/{max} microdollars)",
                    subject.kind, window.window_seconds, state.cost_microdollars
                ),
                state,
            }));
        }
    }
    Ok(None)
}

#[derive(Debug)]
pub struct PostUpdateParams<'a> {
    pub user_id: &'a UserId,
    pub windows: &'a [QuotaWindow],
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_microdollars: i64,
}

pub async fn post_update_tokens(db: &DbPool, params: PostUpdateParams<'_>) {
    if params.windows.is_empty() {
        return;
    }
    let repo = match AiQuotaBucketRepository::new(db) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "quota repo init failed in post_update");
            return;
        },
    };
    let pool = match db.pool_arc() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "quota pool init failed in post_update");
            return;
        },
    };
    let now = Utc::now();
    for window in params.windows {
        let Some(subject) = resolve_subject(window, params.user_id, &pool).await else {
            continue;
        };
        let window_start = align_window(now, window.window_seconds);
        if let Err(e) = repo
            .increment(IncrementParams {
                subject_kind: subject.kind,
                subject_id: &subject.id,
                window_seconds: window.window_seconds,
                window_start,
                delta: QuotaBucketDelta {
                    requests: 0,
                    input_tokens: i64::from(params.input_tokens),
                    output_tokens: i64::from(params.output_tokens),
                    cost_microdollars: params.cost_microdollars,
                },
            })
            .await
        {
            tracing::warn!(error = %e, window_seconds = window.window_seconds, "quota post_update failed");
        }
    }
}

fn align_window(now: DateTime<Utc>, window_seconds: i32) -> DateTime<Utc> {
    let secs = now.timestamp();
    let w = i64::from(window_seconds.max(1));
    let aligned = (secs / w) * w;
    Utc.timestamp_opt(aligned, 0).single().unwrap_or(now)
}
