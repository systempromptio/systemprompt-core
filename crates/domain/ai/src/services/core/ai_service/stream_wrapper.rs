use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use uuid::Uuid;

use crate::models::RequestStatus;
use crate::models::ai::{AiRequest, AiResponse};
use crate::services::core::request_storage::{RequestStorage, StoreParams};
use crate::services::providers::ModelPricing;
use systemprompt_models::ai::StreamChunk;

pub struct StreamStorageParams {
    pub inner: Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>,
    pub storage: RequestStorage,
    pub request: AiRequest,
    pub request_id: Uuid,
    pub start: std::time::Instant,
    pub provider: String,
    pub model: String,
    pub pricing: ModelPricing,
}

pub struct StreamStorageWrapper {
    inner: Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>,
    storage: RequestStorage,
    request: AiRequest,
    request_id: Uuid,
    start: std::time::Instant,
    provider: String,
    model: String,
    pricing: ModelPricing,
    accumulated: String,
    completed: bool,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
    tokens_used: Option<u32>,
    cache_read_tokens: Option<u32>,
    cache_creation_tokens: Option<u32>,
    finish_reason: Option<String>,
}

impl StreamStorageWrapper {
    pub fn new(params: StreamStorageParams) -> Self {
        Self {
            inner: params.inner,
            storage: params.storage,
            request: params.request,
            request_id: params.request_id,
            start: params.start,
            provider: params.provider,
            model: params.model,
            pricing: params.pricing,
            accumulated: String::new(),
            completed: false,
            input_tokens: None,
            output_tokens: None,
            tokens_used: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            finish_reason: None,
        }
    }

    fn capture_usage(&mut self, chunk: StreamChunk) {
        if let StreamChunk::Usage {
            input_tokens,
            output_tokens,
            tokens_used,
            cache_read_tokens,
            cache_creation_tokens,
            finish_reason,
        } = chunk
        {
            if let Some(v) = input_tokens {
                self.input_tokens = Some(self.input_tokens.unwrap_or(0) + v);
            }
            if let Some(v) = output_tokens {
                self.output_tokens = Some(self.output_tokens.unwrap_or(0) + v);
            }
            if let Some(v) = tokens_used {
                self.tokens_used = Some(v);
            }
            if cache_read_tokens.is_some() {
                self.cache_read_tokens = cache_read_tokens;
            }
            if cache_creation_tokens.is_some() {
                self.cache_creation_tokens = cache_creation_tokens;
            }
            if finish_reason.is_some() {
                self.finish_reason = finish_reason;
            }
        }
    }

    fn calculate_cost(&self) -> i64 {
        let input = f64::from(self.input_tokens.unwrap_or(0));
        let output = f64::from(self.output_tokens.unwrap_or(0));
        let input_cost = (input / 1000.0) * f64::from(self.pricing.input_cost_per_1k);
        let output_cost = (output / 1000.0) * f64::from(self.pricing.output_cost_per_1k);
        ((input_cost + output_cost) * 1_000_000.0).round() as i64
    }

    fn build_response(&self) -> AiResponse {
        let mut response = AiResponse::new(
            self.request_id,
            self.accumulated.clone(),
            self.provider.clone(),
            self.model.clone(),
        )
        .with_latency(self.start.elapsed().as_millis() as u64)
        .with_streaming(true);

        response.input_tokens = self.input_tokens;
        response.output_tokens = self.output_tokens;
        response.tokens_used = self.tokens_used;
        response.finish_reason.clone_from(&self.finish_reason);
        response.cache_hit = self.cache_read_tokens.is_some_and(|t| t > 0);
        response.cache_read_tokens = self.cache_read_tokens;
        response.cache_creation_tokens = self.cache_creation_tokens;

        response
    }

    fn store_completion(&self) {
        let response = self.build_response();
        let cost = self.calculate_cost();

        self.storage.store(&StoreParams {
            request: &self.request,
            response: &response,
            context: &self.request.context,
            status: RequestStatus::Completed,
            error_message: None,
            cost_microdollars: cost,
        });
    }

    fn store_error(&self, error: &anyhow::Error) {
        let response = self.build_response();

        self.storage.store(&StoreParams {
            request: &self.request,
            response: &response,
            context: &self.request.context,
            status: RequestStatus::Failed,
            error_message: Some(&error.to_string()),
            cost_microdollars: 0,
        });
    }
}

impl Stream for StreamStorageWrapper {
    type Item = anyhow::Result<StreamChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => match chunk {
                StreamChunk::Text(ref text) => {
                    self.accumulated.push_str(text);
                    Poll::Ready(Some(Ok(chunk)))
                },
                usage @ StreamChunk::Usage { .. } => {
                    self.capture_usage(usage);
                    cx.waker().wake_by_ref();
                    Poll::Pending
                },
            },
            Poll::Ready(Some(Err(e))) => {
                if !self.completed {
                    self.completed = true;
                    self.store_error(&e);
                }
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(None) => {
                if !self.completed {
                    self.completed = true;
                    self.store_completion();
                }
                Poll::Ready(None)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
