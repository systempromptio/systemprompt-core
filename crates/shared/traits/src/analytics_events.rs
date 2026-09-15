//! Persistence contract for analytics ingestion into the logging-owned event
//! store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{SessionId, UserId};

use crate::RepositoryError;

#[derive(Debug, Clone)]
pub struct AnalyticsEventRecord {
    pub id: String,
    pub user_id: UserId,
    pub session_id: SessionId,
    pub event_type: String,
    pub event_category: String,
    pub page_url: String,
    pub event_data: serde_json::Value,
}

/// Logging-owned ingestion and authoritative reads for behavioral checks.
///
/// Batch persistence is atomic and preserves caller-assigned event IDs.
/// Injected as `dyn AnalyticsEventStore`, hence `#[async_trait]`.
#[async_trait]
pub trait AnalyticsEventStore: Send + Sync + std::fmt::Debug {
    async fn persist_events(&self, events: &[AnalyticsEventRecord]) -> Result<(), RepositoryError>;

    async fn get_endpoint_sequence(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<String>, RepositoryError>;

    async fn get_request_timestamps(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<chrono::DateTime<chrono::Utc>>, RepositoryError>;

    async fn has_analytics_events(&self, session_id: &SessionId) -> Result<bool, RepositoryError>;
}

pub type DynAnalyticsEventStore = Arc<dyn AnalyticsEventStore>;
