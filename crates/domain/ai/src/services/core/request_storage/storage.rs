//! Request storage facade persisting AI requests and messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::error::AiError;
use crate::models::RequestStatus;
use crate::models::ai::{AiRequest, AiResponse};
use crate::repository::AiRequestRepository;
use systemprompt_models::RequestContext;
use systemprompt_traits::{AnalyticsEventPublisher, DynAiSessionProvider};

use super::record_builder::{
    BuildRecordParams, build_record, extract_messages, extract_tool_calls,
};
use super::writes::{
    ensure_session_exists, store_messages, store_request, store_tool_calls, update_session_usage,
};

#[derive(Debug)]
pub struct StoreParams<'a> {
    pub request: &'a AiRequest,
    pub response: &'a AiResponse,
    pub context: &'a RequestContext,
    pub status: RequestStatus,
    pub error_message: Option<&'a str>,
    pub cost_microdollars: i64,
}

#[derive(Clone)]
pub struct RequestStorage {
    ai_request_repo: AiRequestRepository,
    session_provider: Option<DynAiSessionProvider>,
    event_publisher: Option<Arc<dyn AnalyticsEventPublisher>>,
}

impl std::fmt::Debug for RequestStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestStorage")
            .field("ai_request_repo", &self.ai_request_repo)
            .field(
                "session_provider",
                &self.session_provider.as_ref().map(|_| "<provider>"),
            )
            .field(
                "event_publisher",
                &self.event_publisher.as_ref().map(|_| "<publisher>"),
            )
            .finish()
    }
}

impl RequestStorage {
    pub fn new(ai_request_repo: AiRequestRepository) -> Self {
        Self {
            ai_request_repo,
            session_provider: None,
            event_publisher: None,
        }
    }

    pub fn with_session_provider(mut self, provider: DynAiSessionProvider) -> Self {
        self.session_provider = Some(provider);
        self
    }

    pub fn with_event_publisher(mut self, publisher: Arc<dyn AnalyticsEventPublisher>) -> Self {
        self.event_publisher = Some(publisher);
        self
    }

    pub async fn store(&self, params: &StoreParams<'_>) -> Result<(), AiError> {
        let record = match build_record(&BuildRecordParams {
            request: params.request,
            response: params.response,
            context: params.context,
            status: params.status,
            error_message: params.error_message,
            cost_microdollars: params.cost_microdollars,
        }) {
            Ok(record) => record,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    request_id = %params.response.request_id,
                    "Skipping ai_requests persistence: record construction failed"
                );
                return Ok(());
            },
        };
        let messages = extract_messages(params.request, params.response, params.status);
        let tool_calls = extract_tool_calls(params.response);

        let user_id = record.user_id.clone();
        let session_id = record.session_id.clone();
        let tokens = record.tokens.tokens_used;
        let cost = record.cost_microdollars;

        // ai_requests.session_id carries a foreign key to user_sessions; the
        // session row must exist before the audit insert or the row is lost —
        // error paths never reach update_session_usage, so this cannot be
        // deferred to it.
        if let (Some(provider), Some(session_id)) =
            (self.session_provider.as_ref(), session_id.as_ref())
        {
            ensure_session_exists(provider.as_ref(), session_id, &user_id).await;
        }

        let db_id = store_request(&self.ai_request_repo, &record).await?;

        store_messages(&self.ai_request_repo, &db_id, messages).await;
        store_tool_calls(&self.ai_request_repo, &db_id, tool_calls).await;

        if let Some(provider) = self.session_provider.as_ref() {
            update_session_usage(
                provider.as_ref(),
                &user_id,
                session_id.as_ref(),
                tokens,
                cost,
            )
            .await;
        }

        if let Some(publisher) = self.event_publisher.as_ref() {
            publisher.publish_analytics_event(
                systemprompt_traits::AnalyticsEvent::AiRequestCompleted {
                    tokens_used: i64::from(tokens.unwrap_or(0)),
                },
            );
        }

        Ok(())
    }
}
