//! Typed error hierarchy for the [`systemprompt-ai`](crate) crate.
//!
//! [`AiError`] is the top-level public error returned by [`crate::services`].
//! It wraps one sub-enum per concern: [`ProviderError`] (talking to an
//! upstream provider), [`RoutingError`] (choosing a provider and model),
//! [`RequestError`] (a request the provider cannot accept), [`ToolError`]
//! (tool schemas and discovery) and [`StorageError`] (persisting generated
//! files), plus the canonical [`RepositoryError`]. Foreign causes convert
//! straight into the sub-enum that owns them, so `?` keeps working.
//!
//! All public service signatures use [`Result<T>`] (i.e. `Result<T, AiError>`).
//! The dyn `AiProvider` seam returns
//! [`AiInferenceError`](systemprompt_models::errors::AiInferenceError); the
//! `From<AiError>` impl in `inference` is the single mapping onto it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod inference;
mod provider;
mod request;
mod response;
mod storage;

use thiserror::Error;

use systemprompt_traits::{AiProviderError, RepositoryError, ToolProviderError};
use systemprompt_wire::error::{WireParseError, WireStreamError};

use crate::services::config::AiConfigError;
use crate::services::storage::StorageConfigError;
use crate::services::upstream::UpstreamTargetError;

pub use provider::{ProviderError, RoutingError};
pub use request::{RequestError, ToolError};
pub use storage::{StorageError, StorageOperation};

#[derive(Debug, Error)]
pub enum AiError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Routing(#[from] RoutingError),

    #[error(transparent)]
    Request(#[from] RequestError),

    #[error(transparent)]
    Tool(#[from] ToolError),

    #[error(transparent)]
    Storage(#[from] StorageError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

impl AiError {
    #[must_use]
    pub fn capability_unsupported(provider: &str, capability: ProviderCapability) -> Self {
        Self::Routing(RoutingError::CapabilityUnsupported {
            provider: systemprompt_identifiers::ProviderId::new(provider),
            capability,
        })
    }
}

macro_rules! route_from {
    ($($source:ty => $outer:ident :: $inner:path),+ $(,)?) => {
        $(impl From<$source> for AiError {
            fn from(err: $source) -> Self {
                Self::$outer($inner(err))
            }
        })+
    };
}

route_from! {
    reqwest::Error => Provider::ProviderError::Http,
    WireStreamError => Provider::ProviderError::Stream,
    WireParseError => Provider::ProviderError::WireParse,
    UpstreamTargetError => Routing::RoutingError::Upstream,
    AiConfigError => Routing::RoutingError::Config,
    systemprompt_config::SecretsBootstrapError => Routing::RoutingError::Secrets,
    ToolProviderError => Tool::ToolError::Provider,
    StorageConfigError => Storage::StorageError::Config,
    base64::DecodeError => Storage::StorageError::ImageDecode,
    AiProviderError => Storage::StorageError::FilePersistence,
}

/// An optional provider feature a request can depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCapability {
    Streaming,
    ToolStreaming,
    GoogleSearch,
    ImageGeneration,
    BatchImageGeneration,
}

impl std::fmt::Display for ProviderCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Streaming => "streaming",
            Self::ToolStreaming => "tool streaming",
            Self::GoogleSearch => "Google Search grounding",
            Self::ImageGeneration => "image generation",
            Self::BatchImageGeneration => "batch image generation",
        })
    }
}

pub type Result<T> = std::result::Result<T, AiError>;
