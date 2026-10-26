//! Registries of gateway upstream adapters and safety scanners, and the
//! [`GatewayEngines`] set that owns one of each engine for a gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use super::protocol::outbound::anthropic::AnthropicOutbound;
use super::protocol::outbound::gemini::GeminiOutbound;
use super::protocol::outbound::openai_chat::OpenAiChatOutbound;
use super::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use super::protocol::outbound::{OutboundAdapter, OutboundAdapterRegistration};
use crate::policies::{
    HeuristicScanner, NullScanner, OverridePolicy, RouteSelectorChain, SafetyConfig, SafetyScanner,
    SafetyScannerRegistration, ScannerFactory, ScannerSettings,
};
use crate::service::failover::{DeploymentLoad, ProviderBreakers};
use systemprompt_client::GuardedClientConfig;
use systemprompt_extension::GatewayGuards;
use systemprompt_security::credential::TokenCache;
use systemprompt_wire::WireProtocol;

pub struct GatewayUpstreamRegistry {
    entries: HashMap<String, Arc<dyn OutboundAdapter>>,
}

impl Default for GatewayUpstreamRegistry {
    fn default() -> Self {
        Self::build()
    }
}

impl std::fmt::Debug for GatewayUpstreamRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayUpstreamRegistry")
            .field("tags", &self.tags())
            .finish()
    }
}

impl GatewayUpstreamRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::build()
    }

    pub fn get(&self, tag: &str) -> Option<&Arc<dyn OutboundAdapter>> {
        self.entries.get(tag)
    }

    pub fn tags(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    pub(super) fn build() -> Self {
        let mut entries: HashMap<String, Arc<dyn OutboundAdapter>> = HashMap::new();

        entries.insert(
            WireProtocol::Anthropic.as_tag().to_owned(),
            Arc::new(AnthropicOutbound),
        );
        entries.insert(
            WireProtocol::OpenAiChat.as_tag().to_owned(),
            Arc::new(OpenAiChatOutbound),
        );
        entries.insert(
            WireProtocol::OpenAiResponses.as_tag().to_owned(),
            Arc::new(OpenAiResponsesOutbound),
        );
        entries.insert(
            WireProtocol::Gemini.as_tag().to_owned(),
            Arc::new(GeminiOutbound),
        );

        for registration in inventory::iter::<OutboundAdapterRegistration> {
            let tag = registration.tag.to_owned();
            if entries.contains_key(&tag) {
                tracing::warn!(
                    tag = %registration.tag,
                    "Extension-registered gateway upstream shadows a built-in"
                );
            }
            entries.insert(tag, (registration.factory)());
        }

        Self { entries }
    }
}

pub struct SafetyScannerRegistry {
    entries: HashMap<String, Arc<dyn ScannerFactory>>,
}

const BUILTIN_SCANNER_NAMES: &[&str] = &["null", "heuristic"];

impl Default for SafetyScannerRegistry {
    fn default() -> Self {
        Self::build()
    }
}

impl std::fmt::Debug for SafetyScannerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafetyScannerRegistry")
            .field("names", &self.names())
            .finish()
    }
}

impl SafetyScannerRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::build()
    }

    pub fn create(&self, name: &str, safety: &SafetyConfig) -> Option<Arc<dyn SafetyScanner>> {
        let settings = safety.settings_for(name);
        self.entries
            .get(name)
            .map(|factory| factory.create(safety, &settings))
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    pub(super) fn build() -> Self {
        fn null_factory(_: &SafetyConfig, _: &ScannerSettings) -> Arc<dyn SafetyScanner> {
            Arc::new(NullScanner)
        }
        fn heuristic_factory(safety: &SafetyConfig, _: &ScannerSettings) -> Arc<dyn SafetyScanner> {
            Arc::new(HeuristicScanner::new(&safety.heuristic))
        }

        let mut entries: HashMap<String, Arc<dyn ScannerFactory>> = HashMap::new();
        entries.insert("null".to_owned(), Arc::new(null_factory));
        entries.insert("heuristic".to_owned(), Arc::new(heuristic_factory));

        for registration in inventory::iter::<SafetyScannerRegistration> {
            let name = registration.name.to_owned();
            if BUILTIN_SCANNER_NAMES.contains(&name.as_str()) {
                tracing::error!(
                    name = %registration.name,
                    "Extension-registered safety scanner uses a built-in name; registration \
                     rejected"
                );
                continue;
            }
            let factory = registration.factory;
            let settings_driven: Arc<dyn ScannerFactory> =
                Arc::new(move |_: &SafetyConfig, settings: &ScannerSettings| factory(settings));
            entries.insert(name, settings_driven);
        }

        Self { entries }
    }
}

/// The engines one gateway owns, built once at the composition root and
/// shared by handle.
///
/// Upstream adapters, safety scanners, the system-prompt override engine, the
/// route selector, the per-provider circuit breakers, the per-deployment
/// in-flight gauge and the upstream HTTP client.
#[derive(Debug)]
pub struct GatewayEngines {
    pub upstream_registry: Arc<GatewayUpstreamRegistry>,
    pub scanner_registry: Arc<SafetyScannerRegistry>,
    pub override_engine: Arc<OverridePolicy>,
    pub route_selector: Arc<RouteSelectorChain>,
    pub breakers: ProviderBreakers,
    pub load: DeploymentLoad,
    pub http: reqwest::Client,
    pub provider_tokens: Arc<TokenCache>,
    pub guards: GatewayGuards,
}

/// Failure to assemble a [`GatewayEngines`] set at boot.
#[derive(Debug, thiserror::Error)]
pub enum GatewayEnginesError {
    #[error("gateway upstream HTTP client could not be built")]
    HttpClient(#[source] reqwest::Error),
}

impl GatewayEngines {
    pub fn new(
        provider_tokens: Arc<TokenCache>,
        guards: GatewayGuards,
    ) -> Result<Self, GatewayEnginesError> {
        let http = systemprompt_client::guarded_client(&GuardedClientConfig {
            timeout: None,
            ..GuardedClientConfig::default()
        })
        .map_err(GatewayEnginesError::HttpClient)?;
        Ok(Self {
            upstream_registry: Arc::default(),
            scanner_registry: Arc::default(),
            override_engine: Arc::default(),
            route_selector: Arc::default(),
            breakers: ProviderBreakers::new(),
            load: DeploymentLoad::default(),
            http,
            provider_tokens,
            guards,
        })
    }
}
