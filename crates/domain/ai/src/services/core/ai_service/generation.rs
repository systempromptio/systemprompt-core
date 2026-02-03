use anyhow::Result;
use uuid::Uuid;

use crate::models::ai::{AiRequest, AiResponse};
use crate::models::RequestStatus;
use crate::services::providers::{AiProvider, GenerationParams, SchemaGenerationParams};

use super::super::request_logging;
use super::super::request_storage::StoreParams;
use super::service::AiService;

#[derive(Debug)]
struct FinalizeResponseParams<'a> {
    result: Result<AiResponse>,
    request_id: Uuid,
    latency_ms: u64,
    request: &'a AiRequest,
    model: &'a str,
}

impl AiService {
    pub async fn generate(&self, request: &AiRequest) -> Result<AiResponse> {
        let request_id = Uuid::new_v4();
        let start = std::time::Instant::now();
        let provider = self.get_provider(request.provider())?;
        let model = request.model();

        request_logging::log_request_start(request_id, request, request.provider(), model);

        let result = self
            .execute_generate(request, provider.as_ref(), model)
            .await;
        let latency_ms = start.elapsed().as_millis() as u64;

        self.finalize_response(FinalizeResponseParams {
            result,
            request_id,
            latency_ms,
            request,
            model,
        })
    }

    async fn execute_generate(
        &self,
        request: &AiRequest,
        provider: &dyn AiProvider,
        model: &str,
    ) -> Result<AiResponse> {
        let base = GenerationParams::new(&request.messages, model, request.max_output_tokens());
        let base = request
            .sampling
            .as_ref()
            .map_or_else(|| base.clone(), |s| base.clone().with_sampling(s));

        if let Some(schema) = request
            .structured_output
            .as_ref()
            .and_then(|s| s.response_format.as_ref().and_then(|f| f.schema()))
        {
            let params = SchemaGenerationParams::new(base, schema.clone());
            return provider.generate_with_schema(params).await;
        }

        provider.generate(base).await
    }

    fn finalize_response(&self, params: FinalizeResponseParams<'_>) -> Result<AiResponse> {
        let FinalizeResponseParams {
            result,
            request_id,
            latency_ms,
            request,
            model,
        } = params;

        match result {
            Ok(mut response) => {
                response.request_id = request_id;
                response.latency_ms = latency_ms;
                let cost = self.estimate_cost(&response);
                self.storage.store(&StoreParams {
                    request,
                    response: &response,
                    context: &request.context,
                    status: RequestStatus::Completed,
                    error_message: None,
                    cost_microdollars: cost,
                });
                request_logging::log_request_success(&response);
                Ok(response)
            },
            Err(e) => {
                let error_response = AiResponse::new(
                    request_id,
                    String::new(),
                    request.provider().to_string(),
                    model.to_string(),
                )
                .with_latency(latency_ms);
                self.storage.store(&StoreParams {
                    request,
                    response: &error_response,
                    context: &request.context,
                    status: RequestStatus::Failed,
                    error_message: Some(&e.to_string()),
                    cost_microdollars: 0,
                });
                request_logging::log_request_error(request_id, request.provider(), latency_ms, &e);
                Err(e)
            },
        }
    }
}
