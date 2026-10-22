//! Image-provider factory, keyed on the endpoint's [`WireProtocol`].
//!
//! `ai.default_image_model` names a catalog model; its first enabled
//! deployment on an image-capable wire with a platform credential is the
//! endpoint images are generated on. Connectivity is that hop resolved into
//! an [`UpstreamTarget`]; the model definitions are every model deployed on
//! the same endpoint, keyed by the upstream name the endpoint is sent, and
//! priced from the deployment. Only the `gemini` and
//! `openai-chat`/`-responses` wires generate images.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_manifest::services::ModelDefinition;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedHop};
use systemprompt_wire::WireProtocol;

use crate::error::{AiError, ProviderCapability, Result};
use crate::services::upstream::{EndpointBinding, UpstreamTarget};

use super::{BoxedImageProvider, GeminiImageProvider, OpenAiImageProvider};

#[derive(Debug)]
pub struct ImageProviderParams<'a> {
    pub catalog: &'a ResolvedCatalog,
    pub hop: &'a ResolvedHop,
    pub target: UpstreamTarget,
}

/// The default image provider and the provider id it is registered under.
pub struct DefaultImageProvider {
    pub provider: String,
    pub client: BoxedImageProvider,
}

impl std::fmt::Debug for DefaultImageProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultImageProvider")
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ImageProviderFactory;

impl ImageProviderFactory {
    pub fn create(params: &ImageProviderParams<'_>) -> Result<BoxedImageProvider> {
        if !params.catalog.is_enabled(params.hop) {
            return Err(AiError::ProviderDisabled {
                provider: params.hop.provider.as_str().to_owned(),
            });
        }
        match params.hop.wire {
            WireProtocol::Gemini => Ok(Self::create_gemini(params)),
            WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => {
                Ok(Self::create_openai(params))
            },
            WireProtocol::Anthropic => Err(AiError::CapabilityUnsupported {
                provider: params.hop.provider.as_str().to_owned(),
                capability: ProviderCapability::ImageGeneration,
            }),
        }
    }

    /// The provider for `ai.default_image_model`, or `None` when it is unset.
    pub fn from_catalog(catalog: &ResolvedCatalog) -> Result<Option<DefaultImageProvider>> {
        let Some(model) = catalog.default_image_model() else {
            return Ok(None);
        };
        let hop = catalog
            .enabled_hops(model.as_str())
            .filter(|hop| Self::supports_image_generation(hop.wire))
            .find(|hop| !hop.credential.is_personal())
            .ok_or_else(|| AiError::NoProviderForModel {
                model: model.as_str().to_owned(),
            })?;
        let binding = EndpointBinding::of_hop(catalog, hop)?;
        let target = UpstreamTarget::from_secrets(binding)?;
        let client = Self::create(&ImageProviderParams {
            catalog,
            hop,
            target,
        })?;
        Ok(Some(DefaultImageProvider {
            provider: hop.provider.as_str().to_owned(),
            client,
        }))
    }

    #[must_use]
    pub const fn supports_image_generation(protocol: WireProtocol) -> bool {
        matches!(
            protocol,
            WireProtocol::Gemini | WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses
        )
    }

    fn create_gemini(params: &ImageProviderParams<'_>) -> BoxedImageProvider {
        Arc::new(
            GeminiImageProvider::with_target(params.target.clone())
                .with_model_definitions(Self::model_definitions(params))
                .with_default_model(params.hop.upstream_model.clone()),
        )
    }

    fn create_openai(params: &ImageProviderParams<'_>) -> BoxedImageProvider {
        Arc::new(
            OpenAiImageProvider::with_target(params.target.clone())
                .with_model_definitions(Self::model_definitions(params))
                .with_default_model(params.hop.upstream_model.clone()),
        )
    }

    fn model_definitions(params: &ImageProviderParams<'_>) -> HashMap<String, ModelDefinition> {
        let endpoint = params.hop.endpoint_ref();
        params
            .catalog
            .models()
            .filter_map(|model| {
                let hop = params
                    .catalog
                    .deployments(model.id.as_str())
                    .iter()
                    .find(|hop| hop.endpoint_ref() == endpoint)?;
                Some((
                    hop.upstream_model.clone(),
                    ModelDefinition {
                        capabilities: model.capabilities,
                        limits: model.limits,
                        pricing: hop.pricing,
                    },
                ))
            })
            .collect()
    }
}
