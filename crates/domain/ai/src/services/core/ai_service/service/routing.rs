//! `AiService` provider selection for a request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{ModelId, ProviderId};

use super::AiService;
use crate::error::Result;
use crate::models::ai::AiRequest;
use crate::services::providers::ProviderClient;

impl AiService {
    pub(in crate::services::core::ai_service) fn get_provider(
        &self,
        request: &AiRequest,
    ) -> Result<Arc<dyn ProviderClient>> {
        let provider = Some(request.provider()).filter(|p| !p.is_empty());
        self.provider_for(provider, request.model())
    }

    pub(in crate::services::core::ai_service) fn provider_for(
        &self,
        provider: Option<&str>,
        model: &str,
    ) -> Result<Arc<dyn ProviderClient>> {
        self.clients
            .for_request(&self.catalog, provider, model)
            .ok_or_else(|| {
                let model = ModelId::new(model);
                let routing = provider.map_or_else(
                    || crate::error::RoutingError::NoProviderForModel {
                        model: model.clone(),
                    },
                    |provider| {
                        let configured = self
                            .clients
                            .iter()
                            .any(|(endpoint, _)| endpoint.provider.as_str() == provider);
                        if configured {
                            crate::error::RoutingError::UnknownModel {
                                provider: ProviderId::new(provider),
                                model: model.clone(),
                            }
                        } else {
                            crate::error::RoutingError::ProviderNotFound {
                                provider: ProviderId::new(provider),
                            }
                        }
                    },
                );
                crate::error::AiError::Routing(routing)
            })
    }
}
