//! `/v1/bridge/profile/usage` — per-user token usage and conversation summary.
//!
//! Returns rolling 24h / 7d / 30d windows of cost + tokens for the JWT
//! subject, the top 5 models by token share, and a conversation summary
//! grouped by model and by agent. Powers the bridge dashboard's profile tab.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use systemprompt_analytics::{AnalyticsResult, CostAnalyticsRepository};
use systemprompt_identifiers::JwtToken;
use systemprompt_models::api::cloud::{
    BridgeProfileUsage, ConversationGroup, ConversationSummary, ModelShare,
    RecentConversationSummary, UsageWindow,
};

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

const TOP_MODELS_LIMIT: i64 = 5;
const TOP_GROUPS_LIMIT: i64 = 10;
const RECENT_LIMIT: i64 = 10;

pub async fn handle(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
) -> Result<Json<BridgeProfileUsage>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_string(),
        )
    })?;
    let claims = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let user_id = claims.user_id.as_str();
    let repo = CostAnalyticsRepository::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = Utc::now();
    let d1_start = now - Duration::days(1);
    let d7_start = now - Duration::days(7);
    let d30_start = now - Duration::days(30);

    let (d1, d7, d30) = tokio::try_join!(
        window(&repo, user_id, d1_start, now, Duration::days(1)),
        window(&repo, user_id, d7_start, now, Duration::days(7)),
        window(&repo, user_id, d30_start, now, Duration::days(30)),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let model_breakdown = repo
        .get_breakdown_by_model_for_user(user_id, d30_start, now, TOP_MODELS_LIMIT)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total_tokens: i64 = model_breakdown.iter().map(|r| r.tokens).sum();
    let top_models: Vec<ModelShare> = model_breakdown
        .into_iter()
        .map(|r| ModelShare {
            token_share: if total_tokens > 0 {
                r.tokens as f64 / total_tokens as f64
            } else {
                0.0
            },
            model: r.name,
            requests: r.requests,
            tokens: r.tokens,
            cost_microdollars: r.cost,
        })
        .collect();

    let conversations = conversation_summary(&repo, user_id, d30_start, now)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(BridgeProfileUsage {
        d1,
        d7,
        d30,
        top_models,
        conversations,
    }))
}

async fn window(
    repo: &CostAnalyticsRepository,
    user_id: &str,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
    span: Duration,
) -> AnalyticsResult<UsageWindow> {
    let summary = repo.get_summary_for_user(user_id, start, end).await?;
    let prev_start = start - span;
    let prev = repo
        .get_previous_cost_for_user(user_id, prev_start, start)
        .await?;
    Ok(UsageWindow {
        requests: summary.requests,
        tokens: summary.tokens.unwrap_or(0),
        cost_microdollars: summary.cost.unwrap_or(0),
        previous_cost_microdollars: prev.cost,
    })
}

async fn conversation_summary(
    repo: &CostAnalyticsRepository,
    user_id: &str,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
) -> AnalyticsResult<ConversationSummary> {
    let total = repo
        .get_context_summary_for_user(user_id, start, end)
        .await?;
    let by_model = repo
        .get_contexts_by_model_for_user(user_id, start, end, TOP_GROUPS_LIMIT)
        .await?;
    let by_agent = repo
        .get_contexts_by_agent_for_user(user_id, start, end, TOP_GROUPS_LIMIT)
        .await?;
    let recent = repo
        .get_recent_contexts_for_user(user_id, end, RECENT_LIMIT)
        .await?;

    let to_group = |r: systemprompt_analytics::ContextGroupRow| ConversationGroup {
        name: r.name,
        conversations: r.conversations,
        ai_requests: r.ai_requests,
    };

    Ok(ConversationSummary {
        total_conversations: total.conversations,
        total_ai_requests: total.ai_requests,
        by_model: by_model.into_iter().map(to_group).collect(),
        by_agent: by_agent.into_iter().map(to_group).collect(),
        recent: recent
            .into_iter()
            .map(|r| RecentConversationSummary {
                context_id: r.context_id,
                last_activity: r.last_activity,
                ai_requests: r.ai_requests,
                model: r.model,
                agent_name: r.agent_name,
            })
            .collect(),
    })
}
