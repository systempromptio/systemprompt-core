//! Raw analytics ingestion through the logging sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::analytics_events::{AnalyticsEventRecord, DynAnalyticsEventStore};

use crate::models::{AnalyticsEventCreated, CreateAnalyticsEventInput};

#[derive(Clone, Debug)]
pub struct AnalyticsEventsRepository {
    event_sink: DynAnalyticsEventStore,
}

impl AnalyticsEventsRepository {
    pub const fn new(event_sink: DynAnalyticsEventStore) -> Self {
        Self { event_sink }
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
