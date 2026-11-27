use async_trait::async_trait;
use systemprompt_engine::run::{
    EnabledExtensions, Endpoint, GovernError, Governor, McpSpec, ModelCall, ModelSpec,
    ResponseAction, ToolCall, Usage, Verdict,
};

/// A Governor that offers no models, enables no extension and allows every
/// tool: an engine built with it refuses every prompt with
/// `ErrorKind::NotSignedIn`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NullGovernor;

#[async_trait]
impl Governor for NullGovernor {
    fn models(&self) -> Vec<ModelSpec> {
        Vec::new()
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        Vec::new()
    }

    async fn authorize(&self, _call: &ModelCall) -> Result<Endpoint, GovernError> {
        Err(GovernError::NotSignedIn)
    }

    async fn response(&self, _call: &ModelCall, _status: u16, _attempt: u32) -> ResponseAction {
        ResponseAction::Continue
    }

    async fn step_finished(&self, _call: &ModelCall, _usage: &Usage) {}

    async fn tool_before(&self, _call: &ToolCall) -> Verdict {
        Verdict::Allow
    }

    fn extensions(&self) -> Option<EnabledExtensions> {
        None
    }

    fn generation(&self) -> u64 {
        0
    }
}
