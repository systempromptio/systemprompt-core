//! Behavioural scoring of a tracked request's session.
//!
//! The request-analytics middleware hands every tracked request's session to
//! a [`SessionBehaviourScorer`] after the response is produced. The analytics
//! extension provides the scorer in the extension state; when no extension
//! provides one, the request is served and no behavioural verdict is
//! recorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::SessionId;

use crate::analytics::AnalyticsResult;

#[derive(Debug, Clone)]
pub struct DetectionSubject {
    pub session_id: SessionId,
    pub fingerprint_hash: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: i64,
}

/// Scores a session from its request history and records the verdict.
///
/// `#[async_trait]` keeps the trait `dyn`-compatible: the middleware holds it
/// as [`DynSessionBehaviourScorer`].
#[async_trait]
pub trait SessionBehaviourScorer: Send + Sync {
    async fn score(&self, subject: DetectionSubject) -> AnalyticsResult<()>;
}

pub type DynSessionBehaviourScorer = Arc<dyn SessionBehaviourScorer>;
