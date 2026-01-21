use std::sync::Arc;

use anyhow::Result;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, SessionId, UserId};

use crate::models::{AnalyticsEventCreated, AnalyticsEventType, CreateAnalyticsEventInput};

#[derive(Clone, Debug)]
pub struct AnalyticsEventsRepository {
    pool: Arc<PgPool>,
}

impl AnalyticsEventsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn create_event(
        &self,
        session_id: &str,
        user_id: &str,
        input: &CreateAnalyticsEventInput,
    ) -> Result<AnalyticsEventCreated> {
        let id = format!("evt_{}", uuid::Uuid::new_v4());
        let event_type = input.event_type.as_str();
        let event_category = input.event_type.category();

        let event_data = Self::build_event_data(input);

        sqlx::query!(
            r#"
            INSERT INTO analytics_events (
                id, user_id, session_id, event_type, event_category,
                severity, endpoint, event_data
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            id,
            user_id,
            session_id,
            event_type,
            event_category,
            "info",
            input.page_url,
            event_data
        )
        .execute(&*self.pool)
        .await?;

        Ok(AnalyticsEventCreated {
            id,
            event_type: event_type.to_string(),
        })
    }

    pub async fn create_events_batch(
        &self,
        session_id: &str,
        user_id: &str,
        inputs: &[CreateAnalyticsEventInput],
    ) -> Result<Vec<AnalyticsEventCreated>> {
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let created = self.create_event(session_id, user_id, input).await?;
            results.push(created);
        }

        Ok(results)
    }

    pub async fn count_events_by_type(
        &self,
        session_id: &str,
        event_type: &AnalyticsEventType,
    ) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM analytics_events
            WHERE session_id = $1 AND event_type = $2
            "#,
            session_id,
            event_type.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(count)
    }

    pub async fn find_by_session(
        &self,
        session_id: &str,
        limit: i64,
    ) -> Result<Vec<StoredAnalyticsEvent>> {
        let events = sqlx::query_as!(
            StoredAnalyticsEvent,
            r#"
            SELECT
                id, user_id, session_id, event_type, event_category,
                endpoint as page_url, event_data, timestamp
            FROM analytics_events
            WHERE session_id = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            session_id,
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
                id, user_id, session_id, event_type, event_category,
                endpoint as page_url, event_data, timestamp
            FROM analytics_events
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
                    "content_id".to_string(),
                    serde_json::json!(content_id.as_str()),
                );
            }
            if let Some(slug) = &input.slug {
                obj.insert("slug".to_string(), serde_json::json!(slug));
            }
            if let Some(referrer) = &input.referrer {
                obj.insert("referrer".to_string(), serde_json::json!(referrer));
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
