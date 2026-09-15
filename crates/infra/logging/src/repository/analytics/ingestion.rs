//! Batched ingestion into the logging-owned analytics event store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::SessionId;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::analytics_events::{AnalyticsEventRecord, AnalyticsEventStore};

use super::AnalyticsRepository;

#[async_trait]
impl AnalyticsEventStore for AnalyticsRepository {
    async fn persist_events(&self, events: &[AnalyticsEventRecord]) -> Result<(), RepositoryError> {
        if events.is_empty() {
            return Ok(());
        }

        let ids: Vec<_> = events.iter().map(|event| event.id.clone()).collect();
        let user_ids: Vec<_> = events
            .iter()
            .map(|event| event.user_id.to_string())
            .collect();
        let session_ids: Vec<_> = events
            .iter()
            .map(|event| event.session_id.to_string())
            .collect();
        let event_types: Vec<_> = events
            .iter()
            .map(|event| event.event_type.clone())
            .collect();
        let event_categories: Vec<_> = events
            .iter()
            .map(|event| event.event_category.clone())
            .collect();
        let severities = vec!["info".to_owned(); events.len()];
        let endpoints: Vec<_> = events.iter().map(|event| event.page_url.clone()).collect();
        let event_datas: Vec<_> = events
            .iter()
            .map(|event| event.event_data.clone())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO analytics_events (id, user_id, session_id, event_type, event_category, severity, endpoint, event_data)
            SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[], $7::text[], $8::jsonb[])
            "#,
            &ids,
            &user_ids,
            &session_ids,
            &event_types,
            &event_categories,
            &severities,
            &endpoints,
            &event_datas
        )
        .execute(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)?;

        Ok(())
    }

    async fn get_endpoint_sequence(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<String>, RepositoryError> {
        let endpoints = sqlx::query_scalar!(
            r#"
        SELECT endpoint
        FROM analytics_events
        WHERE session_id = $1
          AND event_type = 'page_view'
        ORDER BY timestamp ASC
        "#,
            session_id.as_str()
        )
        .fetch_all(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)?;
        Ok(endpoints.into_iter().flatten().collect())
    }

    async fn get_request_timestamps(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<chrono::DateTime<chrono::Utc>>, RepositoryError> {
        sqlx::query_scalar!(
            r#"
        SELECT timestamp as "timestamp!"
        FROM analytics_events
        WHERE session_id = $1
        ORDER BY timestamp ASC
        "#,
            session_id.as_str()
        )
        .fetch_all(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)
    }

    async fn has_analytics_events(&self, session_id: &SessionId) -> Result<bool, RepositoryError> {
        sqlx::query_scalar!(
            r#"
        SELECT EXISTS(
            SELECT 1 FROM analytics_events WHERE session_id = $1
        ) as "exists!"
        "#,
            session_id.as_str()
        )
        .fetch_one(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)
    }
}
