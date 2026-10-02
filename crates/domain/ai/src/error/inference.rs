//! The single mapping from [`AiError`] onto the `AiProvider` seam's
//! [`AiInferenceError`](systemprompt_models::errors::AiInferenceError).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::AiError;

impl From<AiError> for systemprompt_models::errors::AiInferenceError {
    fn from(err: AiError) -> Self {
        match err {
            AiError::ModelNotSpecified { ref provider }
            | AiError::EmptyProviderResponse { ref provider } => Self::Provider {
                provider: provider.clone(),
                message: err.to_string(),
            },
            AiError::ProviderError { provider, message } => Self::Provider { provider, message },
            AiError::NoProviderForModel { model } => Self::NoProviderForModel { model },
            AiError::RateLimit { provider, details } => Self::RateLimited { provider, details },
            AiError::AuthenticationFailed { provider } => Self::AuthenticationFailed { provider },
            AiError::HttpStatus { ref provider, .. }
            | AiError::Timeout { ref provider, .. }
            | AiError::CircuitOpen { ref provider }
            | AiError::DependencyUnavailable { ref provider } => Self::Unavailable {
                provider: provider.clone(),
                message: err.to_string(),
            },
            AiError::MissingMetadata { .. }
            | AiError::MissingUserContext
            | AiError::InvalidToolSchema { .. }
            | AiError::StructuredOutputFailed { .. }
            | AiError::MessageSerializationFailed
            | AiError::MissingToolField { .. }
            | AiError::EmptyToolDescription { .. }
            | AiError::InvalidInput(_)
            | AiError::ImageDecode(_)
            | AiError::InvalidFileId(_)
            | AiError::WireParse(_)
            | AiError::CapabilityUnsupported { .. }
            | AiError::NoProviderWithCapability { .. } => Self::InvalidRequest(Box::new(err)),
            AiError::NoToolCalls
            | AiError::McpServiceNotFound { .. }
            | AiError::McpAuthenticationMissing { .. }
            | AiError::ServiceAuthCheckFailed { .. }
            | AiError::ToolDiscovery(_)
            | AiError::ToolProvider(_) => Self::Tool(Box::new(err)),
            AiError::UnknownModel { .. }
            | AiError::AuthenticationRequired { .. }
            | AiError::ConfigurationError { .. }
            | AiError::Secrets(_)
            | AiError::StorageConfig(_)
            | AiError::Upstream(_)
            | AiError::Config(_)
            | AiError::ProviderNotFound { .. }
            | AiError::ProviderDisabled { .. } => Self::Configuration(Box::new(err)),
            AiError::Repository(_)
            | AiError::FilePersistence(_)
            | AiError::Storage { .. }
            | AiError::ImageTooLarge { .. } => Self::Storage(Box::new(err)),
            AiError::Stream(_)
            | AiError::SerializationError(_)
            | AiError::Http(_)
            | AiError::Io(_)
            | AiError::Regex(_) => Self::Internal(Box::new(err)),
        }
    }
}
