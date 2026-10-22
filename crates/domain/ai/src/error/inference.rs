//! The single mapping from [`AiError`] onto the `AiProvider` seam's
//! [`AiInferenceError`](systemprompt_models::errors::AiInferenceError).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::errors::AiInferenceError;

use super::{AiError, ProviderError, RequestError, RoutingError, StorageError, ToolError};

impl From<AiError> for AiInferenceError {
    fn from(err: AiError) -> Self {
        match err {
            AiError::Provider(e) => from_provider(e),
            AiError::Routing(e) => from_routing(e),
            AiError::Request(ref e) => {
                let provider = match e {
                    RequestError::PromptTooLong { provider, .. }
                    | RequestError::UnsupportedResolution { provider, .. }
                    | RequestError::UnsupportedAspectRatio { provider, .. } => provider.to_string(),
                };
                Self::Provider {
                    provider,
                    message: err.to_string(),
                }
            },
            AiError::Tool(ToolError::MissingField { .. } | ToolError::EmptyDescription { .. })
            | AiError::Storage(StorageError::ImageDecode(_) | StorageError::InvalidFileId(_)) => {
                Self::InvalidRequest(Box::new(err))
            },
            AiError::Tool(ToolError::DiscoveryIncomplete { .. } | ToolError::Provider(_)) => {
                Self::Tool(Box::new(err))
            },
            AiError::Storage(StorageError::Config(_)) => Self::Configuration(Box::new(err)),
            AiError::Storage(_) | AiError::Repository(_) => Self::Storage(Box::new(err)),
            AiError::Serialization(_) | AiError::Io(_) | AiError::Regex(_) => {
                Self::Internal(Box::new(err))
            },
        }
    }
}

fn from_provider(err: ProviderError) -> AiInferenceError {
    match err {
        ProviderError::EmptyResponse { ref provider }
        | ProviderError::ImageWithheld { ref provider }
        | ProviderError::NoImageData { ref provider }
        | ProviderError::NoStructuredOutput { ref provider } => AiInferenceError::Provider {
            provider: provider.to_string(),
            message: err.to_string(),
        },
        ProviderError::HttpStatus { ref provider, .. }
        | ProviderError::Timeout { ref provider, .. }
        | ProviderError::CircuitOpen { ref provider }
        | ProviderError::DependencyUnavailable { ref provider } => AiInferenceError::Unavailable {
            provider: provider.to_string(),
            message: err.to_string(),
        },
        ProviderError::WireParse(_) => AiInferenceError::InvalidRequest(Box::new(err)),
        ProviderError::Http(_) | ProviderError::Stream(_) => {
            AiInferenceError::Internal(Box::new(err))
        },
    }
}

fn from_routing(err: RoutingError) -> AiInferenceError {
    match err {
        RoutingError::NoProviderForModel { model } => AiInferenceError::NoProviderForModel {
            model: model.to_string(),
        },
        RoutingError::ModelUnsupported { ref provider, .. } => AiInferenceError::Provider {
            provider: provider.to_string(),
            message: err.to_string(),
        },
        RoutingError::CapabilityUnsupported { .. }
        | RoutingError::NoProviderWithCapability { .. } => {
            AiInferenceError::InvalidRequest(Box::new(err))
        },
        RoutingError::NoDefaultProvider
        | RoutingError::ProviderNotFound { .. }
        | RoutingError::ProviderDisabled { .. }
        | RoutingError::UnknownModel { .. }
        | RoutingError::Upstream(_)
        | RoutingError::Config(_)
        | RoutingError::Secrets(_) => AiInferenceError::Configuration(Box::new(err)),
    }
}
