//! [`RequestAnalytics`]: the write the API's request-analytics middleware
//! records one served request through. The analytics member provides it open
//! (it appends an event row and reads nothing back); with no provider the
//! request is served and no event is recorded — recording never decides
//! whether a request is served.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};

use crate::BoxedSource;

#[derive(Debug, Clone)]
pub struct RequestAnalyticsEvent {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub context_id: ContextId,
    pub event_type: String,
    pub event_category: String,
    pub severity: String,
    pub endpoint: Option<String>,
    pub error_code: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub task_id: Option<TaskId>,
    pub message: Option<String>,
    // JSON: analytics event `metadata` — open per-request detail, stored as JSONB.
    pub metadata: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
#[error("the request analytics event was not recorded")]
pub struct RequestAnalyticsError(#[source] pub BoxedSource);

/// Records one served request. `#[async_trait]` keeps the trait
/// `dyn`-compatible.
#[async_trait]
pub trait RequestAnalytics: Send + Sync {
    async fn record(&self, event: RequestAnalyticsEvent) -> Result<(), RequestAnalyticsError>;
}

pub type DynRequestAnalytics = Arc<dyn RequestAnalytics>;
