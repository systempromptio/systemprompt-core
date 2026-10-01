//! Session signals analytics composes across owners.
//!
//! Plain session reads and writes belong to the users domain and are reached
//! through [`systemprompt_traits::SessionStore`] directly. What lives here is
//! only what analytics adds on top: behavioural-detector inputs drawn from the
//! analytics event store and the content catalogue, engagement counts over the
//! sessions a fingerprint owns, and geolocation backfill for sessions that
//! arrived without it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod engagement_queries;
mod geo;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{DynAnalyticsEventStore, DynContentCatalogStats, DynSessionStore};

use crate::{AnalyticsError, Result};

#[derive(Clone)]
pub struct SessionSignalsRepository {
    write_pool: Arc<PgPool>,
    owner: DynSessionStore,
    events: DynAnalyticsEventStore,
    content: DynContentCatalogStats,
}

impl std::fmt::Debug for SessionSignalsRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSignalsRepository")
            .finish_non_exhaustive()
    }
}

impl SessionSignalsRepository {
    pub fn new(
        db: &DbPool,
        owner: DynSessionStore,
        events: DynAnalyticsEventStore,
        content: DynContentCatalogStats,
    ) -> Result<Self> {
        Ok(Self {
            write_pool: db.write_pool_arc()?,
            owner,
            events,
            content,
        })
    }

    pub async fn get_endpoint_sequence(&self, session_id: &SessionId) -> Result<Vec<String>> {
        self.events
            .get_endpoint_sequence(session_id)
            .await
            .map_err(AnalyticsError::from)
    }

    pub async fn get_request_timestamps(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<DateTime<Utc>>> {
        self.events
            .get_request_timestamps(session_id)
            .await
            .map_err(AnalyticsError::from)
    }

    pub async fn has_analytics_events(&self, session_id: &SessionId) -> Result<bool> {
        self.events
            .has_analytics_events(session_id)
            .await
            .map_err(AnalyticsError::from)
    }

    pub async fn get_total_content_pages(&self) -> Result<i64> {
        self.content.count_public_pages().await.map_err(Into::into)
    }

    pub async fn count_engagement_events_by_fingerprint(
        &self,
        fingerprint_hash: &str,
        window_days: i64,
    ) -> Result<i64> {
        let session_ids = self
            .owner
            .fingerprint_session_ids(fingerprint_hash, window_days)
            .await
            .map_err(AnalyticsError::from)?
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        engagement_queries::count_engagement_events_for_sessions(&self.write_pool, &session_ids)
            .await
    }

    pub async fn count_sessions_missing_geo(&self) -> Result<i64> {
        self.owner
            .count_sessions_missing_geo()
            .await
            .map_err(AnalyticsError::from)
    }
}
