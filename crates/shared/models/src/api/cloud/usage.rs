//! Cloud usage and conversation-analytics DTOs surfaced in bridge profile
//! reports.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ContextId;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct UsageWindow {
    pub requests: i64,
    pub tokens: i64,
    pub cost_microdollars: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_cost_microdollars: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelShare {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost_microdollars: i64,
    pub token_share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationGroup {
    pub name: String,
    pub conversations: i64,
    pub ai_requests: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentConversationSummary {
    pub context_id: ContextId,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub ai_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub total_conversations: i64,
    pub total_ai_requests: i64,
    #[serde(default)]
    pub by_model: Vec<ConversationGroup>,
    #[serde(default)]
    pub by_agent: Vec<ConversationGroup>,
    #[serde(default)]
    pub recent: Vec<RecentConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProfileUsage {
    pub d1: UsageWindow,
    pub d7: UsageWindow,
    pub d30: UsageWindow,
    #[serde(default)]
    pub top_models: Vec<ModelShare>,
    #[serde(default)]
    pub conversations: ConversationSummary,
}
