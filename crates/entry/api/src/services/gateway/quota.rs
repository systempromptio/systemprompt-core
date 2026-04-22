use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use systemprompt_ai::repository::{
    AiQuotaBucketRepository, IncrementParams, QuotaBucketDelta, QuotaBucketState,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{TenantId, UserId};

use super::policy::QuotaWindow;

#[derive(Debug, Clone, Copy)]
pub struct QuotaDecision {
    pub allow: bool,
    pub window_seconds: i32,
    pub limit_requests: Option<i64>,
    pub limit_input_tokens: Option<i64>,
    pub limit_output_tokens: Option<i64>,
    pub state: QuotaBucketState,
}

pub async fn precheck_and_reserve(
    db: &DbPool,
    tenant_id: Option<&TenantId>,
    user_id: &UserId,
    windows: &[QuotaWindow],
) -> Result<Option<QuotaDecision>> {
    if windows.is_empty() {
        return Ok(None);
    }
    let repo =
        AiQuotaBucketRepository::new(db).map_err(|e| anyhow::anyhow!("quota repo init: {e}"))?;

    let now = Utc::now();
    for window in windows {
        let window_start = align_window(now, window.window_seconds);
        let state = repo
            .increment(IncrementParams {
                tenant_id,
                user_id,
                window_seconds: window.window_seconds,
                window_start,
                delta: QuotaBucketDelta {
                    requests: 1,
                    input_tokens: 0,
                    output_tokens: 0,
                },
            })
            .await?;

        if let Some(max) = window.max_requests {
            if state.requests > max {
                return Ok(Some(QuotaDecision {
                    allow: false,
                    window_seconds: window.window_seconds,
                    limit_requests: Some(max),
                    limit_input_tokens: window.max_input_tokens,
                    limit_output_tokens: window.max_output_tokens,
                    state,
                }));
            }
        }
    }
    Ok(None)
}

#[derive(Debug)]
pub struct PostUpdateParams<'a> {
    pub tenant_id: Option<&'a TenantId>,
    pub user_id: &'a UserId,
    pub windows: &'a [QuotaWindow],
    pub input_tokens: u32,
    pub output_tokens: u32,
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
    let now = Utc::now();
    for window in params.windows {
        let window_start = align_window(now, window.window_seconds);
        if let Err(e) = repo
            .increment(IncrementParams {
                tenant_id: params.tenant_id,
                user_id: params.user_id,
                window_seconds: window.window_seconds,
                window_start,
                delta: QuotaBucketDelta {
                    requests: 0,
                    input_tokens: i64::from(params.input_tokens),
                    output_tokens: i64::from(params.output_tokens),
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
