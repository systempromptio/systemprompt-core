//! Per-user token usage and conversation summary for the profile surfaces.
//!
//! Single source of truth for the rolling 24h / 7d / 30d usage windows, the
//! top models by token share, and the conversation summary. Both the
//! `/v1/bridge/profile/usage` route and the server-rendered admin profile page
//! read through here, so the two surfaces cannot drift apart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::api::cloud::{
    BridgeProfileUsage, ConversationGroup, ConversationSummary, ModelShare,
    RecentConversationSummary, UsageWindow,
};

use crate::error::Result;
use crate::repository::CostAnalyticsRepository;

const TOP_MODELS_LIMIT: i64 = 5;
const TOP_GROUPS_LIMIT: i64 = 10;
const RECENT_LIMIT: i64 = 10;

/// Reads the profile usage windows for one user.
///
/// Cheap to clone — it holds the shared pool handle, so it can be built once
/// at a router's composition root and injected into handlers.
#[derive(Debug, Clone)]
pub struct ProfileUsageService {
    cost_repo: CostAnalyticsRepository,
}

impl ProfileUsageService {
    #[must_use]
    pub const fn new(cost_repo: CostAnalyticsRepository) -> Self {
        Self { cost_repo }
    }

    #[must_use]
    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self::new(CostAnalyticsRepository::from_pool(pool))
    }

    // Why: Everything the profile surfaces render, for one user.
    //
    // `now` is a parameter rather than `Utc::now()` so all three windows are
    // computed against a single instant, and so the result is testable.
    pub async fn get_profile_usage(
        &self,
        user_id: &UserId,
        now: DateTime<Utc>,
    ) -> Result<BridgeProfileUsage> {
        let repo = &self.cost_repo;
        let d30_start = now - Duration::days(30);

        let (d1, d7, d30) = tokio::try_join!(
            self.get_usage_window(user_id, now, Duration::days(1)),
            self.get_usage_window(user_id, now, Duration::days(7)),
            self.get_usage_window(user_id, now, Duration::days(30)),
        )?;

        let top_models = self.list_top_models(user_id, d30_start, now).await?;

        let total = repo
            .get_context_summary_for_user(user_id, d30_start, now)
            .await?;
        let by_model = repo
            .get_contexts_by_model_for_user(user_id, d30_start, now, TOP_GROUPS_LIMIT)
            .await?;
        let by_agent = repo
            .get_contexts_by_agent_for_user(user_id, d30_start, now, TOP_GROUPS_LIMIT)
            .await?;
        let recent = repo
            .get_recent_contexts_for_user(user_id, now, RECENT_LIMIT)
            .await?;

        let to_group = |r: crate::models::ContextGroupRow| ConversationGroup {
            name: r.name,
            conversations: r.conversations,
            ai_requests: r.ai_requests,
        };

        Ok(BridgeProfileUsage {
            d1,
            d7,
            d30,
            top_models,
            conversations: ConversationSummary {
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
                        context_name: r.context_name,
                    })
                    .collect(),
            },
        })
    }

    // Why: One rolling window ending at `now`, carrying the preceding window's
    // cost so a caller can render a delta.
    pub async fn get_usage_window(
        &self,
        user_id: &UserId,
        now: DateTime<Utc>,
        span: Duration,
    ) -> Result<UsageWindow> {
        let start = now - span;
        let summary = self
            .cost_repo
            .get_summary_for_user(user_id, start, now)
            .await?;
        let prev = self
            .cost_repo
            .get_previous_cost_for_user(user_id, start - span, start)
            .await?;
        Ok(UsageWindow {
            requests: summary.requests,
            tokens: summary.tokens.unwrap_or(0),
            cost_microdollars: summary.cost.unwrap_or(0),
            previous_cost_microdollars: prev.cost,
        })
    }

    pub async fn list_top_models(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ModelShare>> {
        let rows = self
            .cost_repo
            .get_breakdown_by_model_for_user(user_id, start, end, TOP_MODELS_LIMIT)
            .await?;

        let total_tokens: i64 = rows.iter().map(|r| r.tokens).sum();
        Ok(rows
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
            .collect())
    }
}
