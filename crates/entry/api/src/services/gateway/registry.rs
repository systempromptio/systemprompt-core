use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use super::upstream::{
    AnthropicCompatibleUpstream, GatewayUpstream, GatewayUpstreamRegistration,
    OpenAiCompatibleUpstream,
};

pub struct GatewayUpstreamRegistry {
    entries: HashMap<String, Arc<dyn GatewayUpstream>>,
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

    pub fn get(&self, tag: &str) -> Option<&Arc<dyn GatewayUpstream>> {
        self.entries.get(tag)
    }

    pub fn tags(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    fn build() -> Self {
        let mut entries: HashMap<String, Arc<dyn GatewayUpstream>> = HashMap::new();

        let anthropic: Arc<dyn GatewayUpstream> = Arc::new(AnthropicCompatibleUpstream);
        let openai: Arc<dyn GatewayUpstream> = Arc::new(OpenAiCompatibleUpstream);

        entries.insert("anthropic".to_string(), Arc::clone(&anthropic));
        entries.insert("minimax".to_string(), Arc::clone(&anthropic));
        entries.insert("openai".to_string(), Arc::clone(&openai));
        entries.insert("moonshot".to_string(), Arc::clone(&openai));
        entries.insert("qwen".to_string(), Arc::clone(&openai));

        for registration in inventory::iter::<GatewayUpstreamRegistration> {
            let tag = registration.tag.to_string();
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
