//! Raw analytics ingestion through the logging sink and event lookups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, SessionId, UserId};
use systemprompt_traits::analytics_events::{AnalyticsEventRecord, DynAnalyticsEventStore};

use crate::models::{AnalyticsEventCreated, AnalyticsEventType, CreateAnalyticsEventInput};

#[derive(Clone, Debug)]
pub struct AnalyticsEventsRepository {
    pool: Arc<PgPool>,
    event_sink: DynAnalyticsEventStore,
}

impl AnalyticsEventsRepository {
    pub fn new(db: &DbPool, event_sink: DynAnalyticsEventStore) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool, event_sink })
    }

    pub async fn create_event(
        &self,
        session_id: &SessionId,
        user_id: &UserId,
        input: &CreateAnalyticsEventInput,
    ) -> Result<AnalyticsEventCreated> {
        let event = Self::build_record(session_id, user_id, input);
        self.event_sink
            .persist_events(std::slice::from_ref(&event))
            .await?;
        Ok(AnalyticsEventCreated {
            id: event.id,
            event_type: event.event_type,
        })
    }

    pub async fn create_events_batch(
        &self,
        session_id: &SessionId,
        user_id: &UserId,
        inputs: &[CreateAnalyticsEventInput],
    ) -> Result<Vec<AnalyticsEventCreated>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let events: Vec<_> = inputs
            .iter()
            .map(|input| Self::build_record(session_id, user_id, input))
            .collect();
        self.event_sink.persist_events(&events).await?;
        Ok(events
            .into_iter()
            .map(|event| AnalyticsEventCreated {
                id: event.id,
                event_type: event.event_type,
            })
            .collect())
    }

    fn build_record(
        session_id: &SessionId,
        user_id: &UserId,
        input: &CreateAnalyticsEventInput,
    ) -> AnalyticsEventRecord {
        AnalyticsEventRecord {
            id: format!("evt_{}", uuid::Uuid::new_v4()),
            user_id: user_id.clone(),
            session_id: session_id.clone(),
            event_type: input.event_type.as_str().to_owned(),
            event_category: input.event_type.category().to_owned(),
            page_url: input.page_url.clone(),
            event_data: Self::build_event_data(input),
        }
    }

    pub async fn count_events_by_type(
        &self,
        session_id: &SessionId,
        event_type: &AnalyticsEventType,
    ) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM analytics_report_analytics_events
            WHERE session_id = $1 AND event_type = $2
            "#,
            session_id.as_str(),
            event_type.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(count)
    }

    pub async fn find_by_session(
        &self,
        session_id: &SessionId,
        limit: i64,
    ) -> Result<Vec<StoredAnalyticsEvent>> {
        let events = sqlx::query_as!(
            StoredAnalyticsEvent,
            r#"
            SELECT
                id,
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                event_type,
                event_category,
                endpoint as page_url,
                event_data,
                timestamp
            FROM analytics_report_analytics_events
            WHERE session_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            session_id.as_str(),
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(events)
    }

    pub async fn find_by_content(
        &self,
        content_id: &ContentId,
        limit: i64,
    ) -> Result<Vec<StoredAnalyticsEvent>> {
        let events = sqlx::query_as!(
            StoredAnalyticsEvent,
            r#"
            SELECT
                id,
                user_id as "user_id: UserId",
                session_id as "session_id: SessionId",
                event_type,
                event_category,
                endpoint as page_url,
                event_data,
                timestamp
            FROM analytics_report_analytics_events
            WHERE event_data->>'content_id' = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            content_id.as_str(),
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(events)
    }

    fn build_event_data(input: &CreateAnalyticsEventInput) -> serde_json::Value {
        let mut data = input.data.clone().unwrap_or(serde_json::json!({}));

        if let Some(obj) = data.as_object_mut() {
            if let Some(content_id) = &input.content_id {
                obj.insert(
                    "content_id".to_owned(),
                    serde_json::json!(content_id.as_str()),
                );
            }
            if let Some(slug) = &input.slug {
                obj.insert("slug".to_owned(), serde_json::json!(slug));
            }
            if let Some(referrer) = &input.referrer {
                obj.insert("referrer".to_owned(), serde_json::json!(referrer));
            }
        }

        data
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredAnalyticsEvent {
    pub id: String,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub event_type: String,
    pub event_category: String,
    pub page_url: Option<String>,
    pub event_data: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
