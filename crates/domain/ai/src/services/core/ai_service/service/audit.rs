//! `AiService` request audit writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::AiService;
use crate::models::RequestStatus;
use crate::models::ai::AiRequest;
use crate::services::core::request_storage::StoreParams;

impl AiService {
    pub(in crate::services::core::ai_service) async fn audit(&self, params: &StoreParams<'_>) {
        if let Err(e) = self.storage.store(params).await {
            tracing::error!(
                error = %e,
                provider = %params.request.provider(),
                model = %params.request.model(),
                status = ?params.status,
                "audit write failed"
            );
        }
    }

    pub(in crate::services::core::ai_service) async fn store_error(
        &self,
        request: &AiRequest,
        request_id: systemprompt_identifiers::AiRequestId,
        latency_ms: u64,
        error_message: String,
    ) {
        use crate::models::ai::AiResponse;

        let error_response = AiResponse::new(
            request_id,
            String::new(),
            request.provider().to_owned(),
            request.model().to_owned(),
        )
        .with_latency(latency_ms);
        self.audit(&StoreParams {
            request,
            response: &error_response,
            context: &request.context,
            status: RequestStatus::Failed,
            error_message: Some(&error_message),
            cost_microdollars: 0,
        })
        .await;
    }
}
