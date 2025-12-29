use std::sync::Arc;

use crate::models::ai::{AiRequest, AiResponse};
use crate::models::{AiRequestRecord, RequestStatus};
use crate::repository::AiRequestRepository;
use systemprompt_core_analytics::SessionRepository;
use systemprompt_models::RequestContext;
use systemprompt_traits::AnalyticsEventPublisher;

use super::async_operations::{
    store_messages_async, store_request_async, store_tool_calls_async, update_session_usage_async,
};
use super::record_builder::{
    build_record, extract_messages, extract_tool_calls, BuildRecordParams,
};

#[derive(Debug)]
pub struct StoreParams<'a> {
    pub request: &'a AiRequest,
    pub response: &'a AiResponse,
    pub context: &'a RequestContext,
    pub status: RequestStatus,
    pub error_message: Option<&'a str>,
    pub cost_cents: i32,
}

pub struct RequestStorage {
    ai_request_repo: AiRequestRepository,
    session_repo: SessionRepository,
    event_publisher: Option<Arc<dyn AnalyticsEventPublisher>>,
}

impl std::fmt::Debug for RequestStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestStorage")
            .field("ai_request_repo", &self.ai_request_repo)
            .field("session_repo", &self.session_repo)
            .field(
                "event_publisher",
                &self.event_publisher.as_ref().map(|_| "<publisher>"),
            )
            .finish()
    }
}

impl RequestStorage {
    pub fn new(ai_request_repo: AiRequestRepository, session_repo: SessionRepository) -> Self {
        Self {
            ai_request_repo,
            session_repo,
            event_publisher: None,
        }
    }

    pub fn with_event_publisher(mut self, publisher: Arc<dyn AnalyticsEventPublisher>) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    pub fn store(&self, params: &StoreParams<'_>) {
        let record = build_record(&BuildRecordParams {
            request: params.request,
            response: params.response,
            context: params.context,
            status: params.status,
            error_message: params.error_message,
            cost_cents: params.cost_cents,
        });
        let messages = extract_messages(params.request, params.response, params.status);
        let tool_calls = extract_tool_calls(params.response);
        self.spawn_storage_task(record, messages, tool_calls);
    }

    fn spawn_storage_task(
        &self,
        record: AiRequestRecord,
        messages: Vec<super::record_builder::MessageData>,
        tool_calls: Vec<super::record_builder::ToolCallData>,
    ) {
        let repo = self.ai_request_repo.clone();
        let session_repo = self.session_repo.clone();
        let user_id = record.user_id.clone();
        let session_id = record.session_id.clone();
        let tokens = record.tokens.tokens_used;
        let cost = record.cost_cents;
        let event_publisher = self.event_publisher.clone();

        tokio::spawn(async move {
            let Some(db_id) = store_request_async(&repo, &record).await else {
                return;
            };

            store_messages_async(&repo, &db_id, messages).await;
            store_tool_calls_async(&repo, &db_id, tool_calls).await;
            update_session_usage_async(&session_repo, &user_id, session_id.as_ref(), tokens, cost)
                .await;

            if let Some(publisher) = event_publisher {
                publisher.publish_analytics_event(
                    systemprompt_traits::AnalyticsEvent::AiRequestCompleted {
                        tokens_used: i64::from(tokens.unwrap_or(0)),
                    },
                );
            }
        });
    }
}
