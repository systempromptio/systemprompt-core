//! [`ResilientProvider`] — an [`AiProvider`] decorator that applies the
//! resilience policy (timeout, retry, circuit breaker, bulkhead) to every call.
//!
//! [`super::provider_factory::ProviderFactory`] wraps each concrete provider in
//! one of these before handing it to `AiService`, so the service and its
//! callers are unaware of the resilience layer. Every trait method is delegated
//! to the inner provider — none rely on the trait's default implementations,
//! since a default would shadow a concrete provider's real override.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::Stream;
use systemprompt_models::services::ResilienceSettings;
use systemprompt_resilience::{
    BreakerConfig, BulkheadConfig, ResilienceConfig, ResilienceError, ResilienceGuard, RetryConfig,
    guarded_stream,
};

use crate::error::{AiError, Result};
use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse, StreamChunk};
use crate::models::tools::ToolCall;
use crate::services::schema::ProviderCapabilities;

use super::provider_trait::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams, ToolResultsParams,
};

type StreamResult = Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>>;

/// Translate the loaded [`ResilienceSettings`] into the runtime form consumed
/// by `systemprompt-resilience`.
#[must_use]
pub fn to_resilience_config(settings: &ResilienceSettings) -> ResilienceConfig {
    ResilienceConfig {
        request_timeout: Duration::from_millis(settings.request_timeout_ms),
        stream_idle_timeout: Duration::from_millis(settings.stream_idle_timeout_ms),
        retry: RetryConfig {
            max_attempts: settings.retry_attempts.max(1),
            base_delay: Duration::from_millis(settings.retry_base_delay_ms),
            max_delay: Duration::from_millis(settings.retry_max_delay_ms),
            jitter: true,
        },
        breaker: BreakerConfig {
            failure_threshold: settings.breaker_failure_threshold.max(1),
            open_cooldown: Duration::from_millis(settings.breaker_open_cooldown_ms),
            half_open_max_probes: settings.breaker_half_open_probes.max(1),
        },
        bulkhead: BulkheadConfig {
            max_concurrent: settings.max_concurrent.max(1),
        },
    }
}

/// An [`AiProvider`] wrapped in a per-provider [`ResilienceGuard`].
pub struct ResilientProvider {
    provider: String,
    inner: Arc<dyn AiProvider>,
    guard: Arc<ResilienceGuard>,
}

impl std::fmt::Debug for ResilientProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilientProvider")
            .field("provider", &self.provider)
            .field("guard", &self.guard)
            .finish_non_exhaustive()
    }
}

impl ResilientProvider {
    /// Wrap `inner`, keying the guard's breaker and bulkhead on the provider
    /// name.
    #[must_use]
    pub fn new(
        provider: impl Into<String>,
        inner: Arc<dyn AiProvider>,
        settings: &ResilienceSettings,
    ) -> Self {
        let provider = provider.into();
        let guard = ResilienceGuard::new(provider.clone(), to_resilience_config(settings));
        Self {
            provider,
            inner,
            guard: Arc::new(guard),
        }
    }

    /// Map a resilience-layer failure back into the provider's error enum.
    fn map_err(&self, err: ResilienceError<AiError>) -> AiError {
        match err {
            ResilienceError::Inner(inner) => inner,
            ResilienceError::CircuitOpen { .. } => AiError::CircuitOpen {
                provider: self.provider.clone(),
            },
            ResilienceError::BulkheadFull { .. } => AiError::DependencyUnavailable {
                provider: self.provider.clone(),
            },
            ResilienceError::Timeout { after } => AiError::Timeout {
                provider: self.provider.clone(),
                after_ms: after.as_millis() as u64,
            },
        }
    }

    /// Run a streaming call: admit it through the guard (bulkhead + breaker)
    /// before opening the stream, then wrap the stream with the per-chunk idle
    /// timeout, holding the bulkhead permit for the stream's lifetime.
    async fn guarded_stream_call(&self, open: impl Future<Output = StreamResult>) -> StreamResult {
        let permit = self
            .guard
            .acquire_permit::<AiError>()
            .map_err(|err| self.map_err(err))?;
        match open.await {
            Ok(stream) => {
                self.guard.breaker().record_success();
                let provider = self.provider.clone();
                let idle = self.guard.config().stream_idle_timeout;
                let wrapped = guarded_stream(stream, idle, permit, move |after| AiError::Timeout {
                    provider: provider.clone(),
                    after_ms: after.as_millis() as u64,
                });
                Ok(Box::pin(wrapped))
            },
            Err(err) => {
                drop(permit);
                self.guard.breaker().record_failure();
                Err(err)
            },
        }
    }
}

#[async_trait]
impl AiProvider for ResilientProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self.inner.as_any()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn supports_model(&self, model: &str) -> bool {
        self.inner.supports_model(model)
    }

    fn supports_sampling(&self, sampling: Option<&SamplingParams>) -> bool {
        self.inner.supports_sampling(sampling)
    }

    fn default_model(&self) -> &str {
        self.inner.default_model()
    }

    fn get_pricing(&self, model: &str) -> ModelPricing {
        self.inner.get_pricing(model)
    }

    fn supports_json_mode(&self) -> bool {
        self.inner.supports_json_mode()
    }

    fn supports_structured_output(&self) -> bool {
        self.inner.supports_structured_output()
    }

    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }

    fn supports_google_search(&self) -> bool {
        self.inner.supports_google_search()
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        self.guard
            .execute(AiError::classify, || self.inner.generate(params.clone()))
            .await
            .map_err(|err| self.map_err(err))
    }

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        self.guard
            .execute(AiError::classify, || {
                self.inner.generate_with_tools(params.clone())
            })
            .await
            .map_err(|err| self.map_err(err))
    }

    async fn generate_with_tool_results(
        &self,
        params: ToolResultsParams<'_>,
    ) -> Result<AiResponse> {
        self.guard
            .execute(AiError::classify, || {
                self.inner.generate_with_tool_results(params.clone())
            })
            .await
            .map_err(|err| self.map_err(err))
    }

    async fn generate_structured(
        &self,
        params: StructuredGenerationParams<'_>,
    ) -> Result<AiResponse> {
        self.guard
            .execute(AiError::classify, || {
                self.inner.generate_structured(params.clone())
            })
            .await
            .map_err(|err| self.map_err(err))
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        self.guard
            .execute(AiError::classify, || {
                self.inner.generate_with_schema(params.clone())
            })
            .await
            .map_err(|err| self.map_err(err))
    }

    async fn generate_with_google_search(
        &self,
        params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        self.guard
            .execute(AiError::classify, || {
                self.inner.generate_with_google_search(params.clone())
            })
            .await
            .map_err(|err| self.map_err(err))
    }

    async fn generate_stream(&self, params: GenerationParams<'_>) -> StreamResult {
        self.guarded_stream_call(self.inner.generate_stream(params))
            .await
    }

    async fn generate_with_tools_stream(&self, params: ToolGenerationParams<'_>) -> StreamResult {
        self.guarded_stream_call(self.inner.generate_with_tools_stream(params))
            .await
    }
}
