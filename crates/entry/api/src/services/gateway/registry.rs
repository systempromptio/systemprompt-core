//! Process-global registry of gateway upstream providers and adapters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::protocol::outbound::anthropic::AnthropicOutbound;
use super::protocol::outbound::gemini::GeminiOutbound;
use super::protocol::outbound::openai_chat::OpenAiChatOutbound;
use super::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use super::protocol::outbound::{OutboundAdapter, OutboundAdapterRegistration};
use systemprompt_ai::{
    HeuristicScanner, NullScanner, SafetyConfig, SafetyScanner, SafetyScannerRegistration,
    ScannerFactory,
};
use systemprompt_models::profile::WireProtocol;

pub struct GatewayUpstreamRegistry {
    entries: HashMap<String, Arc<dyn OutboundAdapter>>,
}

impl std::fmt::Debug for GatewayUpstreamRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayUpstreamRegistry")
            .field("tags", &self.tags())
            .finish()
    }
}

impl GatewayUpstreamRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<GatewayUpstreamRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::build)
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

impl std::fmt::Debug for SafetyScannerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafetyScannerRegistry")
            .field("names", &self.names())
            .finish()
    }
}

impl SafetyScannerRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<SafetyScannerRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::build)
    }

    pub fn create(&self, name: &str, safety: &SafetyConfig) -> Option<Arc<dyn SafetyScanner>> {
        self.entries.get(name).map(|factory| factory.create(safety))
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    pub(super) fn build() -> Self {
        fn null_factory(_: &SafetyConfig) -> Arc<dyn SafetyScanner> {
            Arc::new(NullScanner)
        }
        fn heuristic_factory(safety: &SafetyConfig) -> Arc<dyn SafetyScanner> {
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
            let config_blind: Arc<dyn ScannerFactory> = Arc::new(move |_: &SafetyConfig| factory());
            entries.insert(name, config_blind);
        }

        Self { entries }
    }
}
