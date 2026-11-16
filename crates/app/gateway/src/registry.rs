//! The gateway's upstream adapters and safety scanners, and the
//! [`GatewayEngines`] set that owns one of each engine for a gateway.
//!
//! The adapter and scanner sets, the override engine and the route selector
//! chain are the built values of the gateway's slots
//! ([`crate::contract`]); [`GatewayEngines::new`] reads each from the
//! resolved state. Each slot is `CoreFirst` and opened by the gateway member,
//! so it is absent exactly when the gateway member is not admitted; building
//! the engines then refuses boot with [`GatewayEnginesError::OwnerSlot`]
//! naming the slot — a half-disabled gateway never serves on defaults or on
//! install entries alone. The gateway guards are the kernel-built
//! [`GatewayGuards`] read from the same state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use super::protocol::outbound::anthropic::AnthropicOutbound;
use super::protocol::outbound::gemini::GeminiOutbound;
use super::protocol::outbound::openai_chat::OpenAiChatOutbound;
use super::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use super::protocol::outbound::{OutboundAdapter, UpstreamBounds, UpstreamHttp};
use crate::policies::{
    HeuristicScanner, NullScanner, OverridePolicy, RouteSelectorChain, SafetyConfig, SafetyScanner,
    ScannerFactory, ScannerSettings,
};
use crate::service::failover::{DeploymentLoad, ProviderBreakers};
use systemprompt_client::GuardedClientConfig;
use systemprompt_config::SecretsService;
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_extension::{GatewayGuards, MissingDependency, Resolved};
use systemprompt_security::credential::TokenCache;
use systemprompt_wire::WireProtocol;

pub struct GatewayUpstreamRegistry {
    entries: HashMap<String, Arc<dyn OutboundAdapter>>,
}

impl Default for GatewayUpstreamRegistry {
    fn default() -> Self {
        Self::builtin()
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
    pub fn builtin() -> Self {
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
        Self { entries }
    }

    #[must_use]
    pub fn is_builtin(tag: &str) -> bool {
        WireProtocol::from_tag(tag).is_some()
    }

    pub fn get(&self, tag: &str) -> Option<&Arc<dyn OutboundAdapter>> {
        self.entries.get(tag)
    }

    pub fn adapter_for(&self, wire: WireProtocol) -> Option<&Arc<dyn OutboundAdapter>> {
        self.entries.get(wire.as_tag())
    }

    pub fn tags(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    pub(crate) fn insert(&mut self, tag: &str, adapter: Arc<dyn OutboundAdapter>) {
        self.entries.insert(tag.to_owned(), adapter);
    }
}

pub struct SafetyScannerRegistry {
    entries: HashMap<String, Arc<dyn ScannerFactory>>,
}

impl Default for SafetyScannerRegistry {
    fn default() -> Self {
        Self::builtin()
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
    pub fn builtin() -> Self {
        fn null_factory(_: &SafetyConfig, _: &ScannerSettings) -> Arc<dyn SafetyScanner> {
            Arc::new(NullScanner)
        }
        fn heuristic_factory(safety: &SafetyConfig, _: &ScannerSettings) -> Arc<dyn SafetyScanner> {
            Arc::new(HeuristicScanner::new(&safety.heuristic))
        }

        let mut entries: HashMap<String, Arc<dyn ScannerFactory>> = HashMap::new();
        entries.insert("null".to_owned(), Arc::new(null_factory));
        entries.insert("heuristic".to_owned(), Arc::new(heuristic_factory));
        Self { entries }
    }

    pub fn create(&self, name: &str, safety: &SafetyConfig) -> Option<Arc<dyn SafetyScanner>> {
        let settings = safety.settings_for(name);
        self.entries
            .get(name)
            .map(|factory| factory.create(safety, &settings))
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    pub(crate) fn insert(&mut self, name: &str, factory: Arc<dyn ScannerFactory>) {
        self.entries.insert(name.to_owned(), factory);
    }
}

/// The engines one gateway owns, built once at the composition root and
/// shared by handle.
///
/// Upstream adapters, safety scanners, the system-prompt override engine, the
/// route selector, the per-provider circuit breakers, the per-deployment
/// in-flight gauge and the upstream HTTP client.
pub struct GatewayEngines {
    pub upstream_registry: Arc<GatewayUpstreamRegistry>,
    pub scanner_registry: Arc<SafetyScannerRegistry>,
    pub override_engine: Arc<OverridePolicy>,
    pub route_selector: Arc<RouteSelectorChain>,
    pub breakers: ProviderBreakers,
    pub load: DeploymentLoad,
    pub http: UpstreamHttp,
    pub provider_tokens: Arc<TokenCache>,
    pub guards: Arc<GatewayGuards>,
    pub secrets: Arc<SecretsService>,
    pub memory_recall: Option<Arc<dyn MemoryRecall>>,
}

impl std::fmt::Debug for GatewayEngines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayEngines")
            .field("upstream_registry", &self.upstream_registry)
            .field("scanner_registry", &self.scanner_registry)
            .field(
                "memory_recall",
                &self.memory_recall.as_ref().map(|recall| recall.name()),
            )
            .finish_non_exhaustive()
    }
}

/// Failure to assemble a [`GatewayEngines`] set at boot.
#[derive(Debug, thiserror::Error)]
pub enum GatewayEnginesError {
    #[error("gateway upstream HTTP client could not be built")]
    HttpClient(#[source] reqwest::Error),
    #[error("gateway guards are missing from the resolved state")]
    Guards(#[source] MissingDependency),
    #[error("the gateway's {slot} slot is absent: its core owner is not admitted")]
    OwnerSlot {
        slot: &'static str,
        #[source]
        source: MissingDependency,
    },
}

fn owner_slot<T: Send + Sync + 'static>(
    r: &Resolved<'_>,
    slot: &'static str,
) -> Result<Arc<T>, GatewayEnginesError> {
    r.get::<Arc<T>>()
        .map_err(|source| GatewayEnginesError::OwnerSlot { slot, source })
}

impl GatewayEngines {
    pub fn new(
        provider_tokens: Arc<TokenCache>,
        r: &Resolved<'_>,
        secrets: Arc<SecretsService>,
    ) -> Result<Self, GatewayEnginesError> {
        let client = systemprompt_client::guarded_client(
            &GuardedClientConfig::for_configured().without_timeout(),
        )
        .map_err(GatewayEnginesError::HttpClient)?;
        let http = UpstreamHttp::new(client, UpstreamBounds::PRODUCTION);
        Ok(Self {
            upstream_registry: owner_slot(r, "outbound")?,
            scanner_registry: owner_slot(r, "safety scanner")?,
            override_engine: owner_slot(r, "override")?,
            route_selector: owner_slot(r, "route selector")?,
            breakers: ProviderBreakers::new(),
            load: DeploymentLoad::default(),
            http,
            provider_tokens,
            guards: r.get().map_err(GatewayEnginesError::Guards)?,
            secrets,
            memory_recall: None,
        })
    }

    #[must_use]
    pub fn with_memory_recall(mut self, recall: Arc<dyn MemoryRecall>) -> Self {
        self.memory_recall = Some(recall);
        self
    }
}
