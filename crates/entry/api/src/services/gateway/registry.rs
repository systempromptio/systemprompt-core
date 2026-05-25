use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::protocol::outbound::anthropic::AnthropicOutbound;
use super::protocol::outbound::openai_chat::OpenAiChatOutbound;
use super::protocol::outbound::openai_responses::OpenAiResponsesOutbound;
use super::protocol::outbound::{OutboundAdapter, OutboundAdapterRegistration};

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

        let anthropic: Arc<dyn OutboundAdapter> = Arc::new(AnthropicOutbound);
        let openai: Arc<dyn OutboundAdapter> = Arc::new(OpenAiChatOutbound);
        let responses: Arc<dyn OutboundAdapter> = Arc::new(OpenAiResponsesOutbound);

        entries.insert("anthropic".to_owned(), Arc::clone(&anthropic));
        entries.insert("minimax".to_owned(), Arc::clone(&anthropic));
        entries.insert("openai".to_owned(), Arc::clone(&openai));
        entries.insert("moonshot".to_owned(), Arc::clone(&openai));
        entries.insert("qwen".to_owned(), Arc::clone(&openai));
        entries.insert("openai-responses".to_owned(), Arc::clone(&responses));

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
