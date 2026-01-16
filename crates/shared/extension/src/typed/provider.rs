use std::sync::Arc;

use systemprompt_provider_contracts::{LlmProvider, ToolProvider};

use crate::types::ExtensionMeta;

pub trait ProviderExtensionTyped: ExtensionMeta {
    fn llm_providers(&self) -> Vec<Arc<dyn LlmProvider>> {
        vec![]
    }

    fn tool_providers(&self) -> Vec<Arc<dyn ToolProvider>> {
        vec![]
    }
}
