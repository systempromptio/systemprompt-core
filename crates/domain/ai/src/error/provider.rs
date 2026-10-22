//! [`ProviderError`] (a call to an upstream provider failed) and
//! [`RoutingError`] (no provider or model could be chosen for a request).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use thiserror::Error;

use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_wire::error::{WireParseError, WireStreamError};

use super::ProviderCapability;
use crate::services::config::AiConfigError;
use crate::services::upstream::UpstreamTargetError;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider {provider} returned empty response")]
    EmptyResponse { provider: ProviderId },

    #[error(
        "Provider {provider} returned no image content - the prompt may have been rejected by \
         content safety filters, the API quota may be exceeded, or the service failed \
         transiently. Please inform the user that image generation failed and the content was \
         created without an image."
    )]
    ImageWithheld { provider: ProviderId },

    #[error("Provider {provider} returned no image data")]
    NoImageData { provider: ProviderId },

    #[error("Provider {provider} returned a structured-output response with no tool_use block")]
    NoStructuredOutput { provider: ProviderId },

    #[error("Provider {provider} returned HTTP {status}: {body}")]
    HttpStatus {
        provider: ProviderId,
        status: u16,
        retry_after: Option<Duration>,
        body: String,
    },

    #[error("Provider {provider} request timed out after {after_ms}ms")]
    Timeout { provider: ProviderId, after_ms: u64 },

    #[error("Circuit breaker open for provider {provider}; failing fast")]
    CircuitOpen { provider: ProviderId },

    #[error("Provider {provider} unavailable: concurrency limit reached")]
    DependencyUnavailable { provider: ProviderId },

    #[error("HTTP request failed: {0}")]
    Http(#[source] reqwest::Error),

    #[error("provider stream failed: {0}")]
    Stream(#[source] WireStreamError),

    #[error(transparent)]
    WireParse(WireParseError),
}

#[derive(Debug, Error)]
pub enum RoutingError {
    #[error("No model specified and no default provider configured")]
    NoDefaultProvider,

    #[error("No configured provider supports model {model}")]
    NoProviderForModel { model: ModelId },

    #[error("provider {provider} is not configured")]
    ProviderNotFound { provider: ProviderId },

    #[error("provider {provider} is disabled")]
    ProviderDisabled { provider: ProviderId },

    #[error("provider {provider} does not support model {model}")]
    ModelUnsupported {
        provider: ProviderId,
        model: ModelId,
    },

    #[error("provider {provider} has no pricing for model {model}; refusing to bill it")]
    UnknownModel {
        provider: ProviderId,
        model: ModelId,
    },

    #[error("provider {provider} does not support {capability}")]
    CapabilityUnsupported {
        provider: ProviderId,
        capability: ProviderCapability,
    },

    #[error("no configured provider supports {capability}")]
    NoProviderWithCapability { capability: ProviderCapability },

    #[error("Upstream provider could not be resolved: {0}")]
    Upstream(#[source] UpstreamTargetError),

    #[error(transparent)]
    Config(AiConfigError),

    #[error(transparent)]
    Secrets(systemprompt_config::SecretsBootstrapError),
}
