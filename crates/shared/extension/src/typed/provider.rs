//! Provider extension trait.

use std::sync::Arc;

use systemprompt_traits::{LlmProvider, ToolProvider};

use crate::types::ExtensionMeta;

pub trait ProviderExtensionTyped: ExtensionMeta {
    fn llm_providers(&self) -> Vec<Arc<dyn LlmProvider>> {
        vec![]
    }

    fn tool_providers(&self) -> Vec<Arc<dyn ToolProvider>> {
        vec![]
    }
}
