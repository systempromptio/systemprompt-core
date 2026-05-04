//! [`ProviderExtensionTyped`] — typed contract for extensions that
//! contribute LLM and tool provider implementations.

use std::sync::Arc;

use systemprompt_provider_contracts::{LlmProvider, ToolProvider};

use crate::types::ExtensionMeta;

/// Typed contract for an extension that contributes LLM and/or tool
/// provider implementations.
pub trait ProviderExtensionTyped: ExtensionMeta {
    /// Returns the LLM providers this extension contributes.
    fn llm_providers(&self) -> Vec<Arc<dyn LlmProvider>> {
        vec![]
    }

    /// Returns the tool providers this extension contributes.
    fn tool_providers(&self) -> Vec<Arc<dyn ToolProvider>> {
        vec![]
    }
}
