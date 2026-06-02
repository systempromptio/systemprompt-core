use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::protocol::outbound::anthropic::AnthropicOutbound;
use super::protocol::outbound::gemini::GeminiOutbound;
use super::protocol::outbound::openai_chat::OpenAiChatOutbound;
use super::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use super::protocol::outbound::{OutboundAdapter, OutboundAdapterRegistration};
use systemprompt_ai::{HeuristicScanner, SafetyScanner, SafetyScannerRegistration};
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

        // Outbound adapters are keyed on the WireProtocol tag, not the provider
        // name: a ProviderEntry's `protocol` selects the wire codec.
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
    entries: HashMap<String, Arc<dyn SafetyScanner>>,
}

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

    pub fn get(&self, name: &str) -> Option<&Arc<dyn SafetyScanner>> {
        self.entries.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    pub(super) fn build() -> Self {
        let mut entries: HashMap<String, Arc<dyn SafetyScanner>> = HashMap::new();
        entries.insert("heuristic".to_owned(), Arc::new(HeuristicScanner));

        for registration in inventory::iter::<SafetyScannerRegistration> {
            let name = registration.name.to_owned();
            if entries.contains_key(&name) {
                tracing::warn!(
                    name = %registration.name,
                    "Extension-registered safety scanner shadows a built-in"
                );
            }
            entries.insert(name, (registration.factory)());
        }

        Self { entries }
    }
}
