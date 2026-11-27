//! The engine's Governor seen through one indirection, so governance can be
//! installed after the slot is built and before the engine is.
//!
//! The engine holds an `Arc<dyn Governor>` for life. A [`GovernorSlot`]
//! delegates every call, the enabled extensions included, to whichever
//! governor is installed; until one is, there are no models, no extensions,
//! and every model call is refused as not signed in. Installing moves
//! [`Governor::generation`] on, so the engine re-reads models and MCP servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use systemprompt_engine::llm::{Endpoint, ModelSpec, Usage};
use systemprompt_engine::run::extension::EnabledExtensions;
use systemprompt_engine::run::{
    GovernError, Governor, McpSpec, ModelCall, ResponseAction, ToolCall, Verdict,
};

const EPOCH_SHIFT: u32 = 48;

#[derive(Debug, Clone, Copy)]
struct SignedOut;

#[async_trait::async_trait]
impl Governor for SignedOut {
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

/// The governor an engine holds for life, delegating to whichever governor is
/// installed.
pub struct GovernorSlot {
    current: RwLock<Arc<dyn Governor>>,
    epoch: AtomicU64,
}

impl fmt::Debug for GovernorSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernorSlot")
            .field("epoch", &self.epoch.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Default for GovernorSlot {
    fn default() -> Self {
        Self {
            current: RwLock::new(Arc::new(SignedOut)),
            epoch: AtomicU64::new(0),
        }
    }
}

impl GovernorSlot {
    pub fn install(&self, governor: Arc<dyn Governor>) {
        *self.current.write() = governor;
        self.epoch.fetch_add(1, Ordering::AcqRel);
        tracing::info!("governor installed");
    }

    pub fn current(&self) -> Arc<dyn Governor> {
        Arc::clone(&self.current.read())
    }
}

#[async_trait::async_trait]
impl Governor for GovernorSlot {
    fn models(&self) -> Vec<ModelSpec> {
        self.current().models()
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        self.current().mcp_servers()
    }

    async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError> {
        let governor = self.current();
        governor.authorize(call).await
    }

    async fn response(&self, call: &ModelCall, status: u16, attempt: u32) -> ResponseAction {
        let governor = self.current();
        governor.response(call, status, attempt).await
    }

    async fn step_finished(&self, call: &ModelCall, usage: &Usage) {
        let governor = self.current();
        governor.step_finished(call, usage).await;
    }

    async fn tool_before(&self, call: &ToolCall) -> Verdict {
        let governor = self.current();
        governor.tool_before(call).await
    }

    fn extensions(&self) -> Option<EnabledExtensions> {
        self.current().extensions()
    }

    fn generation(&self) -> u64 {
        let epoch = self.epoch.load(Ordering::Acquire);
        (epoch << EPOCH_SHIFT).wrapping_add(self.current().generation())
    }

    fn governed_by(&self) -> Option<String> {
        self.current().governed_by()
    }
}
