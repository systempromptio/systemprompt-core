//! The engine half of an extension: what it adds to sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use harness_config::{AgentDef, Command, Paths};

use super::contribution::{ExtensionTool, PromptContext, PromptSection};
use super::hooks::{CallAugmenter, GovernorObserver, ToolReviewer};
use super::metadata::ExtensionMetadata;
use crate::governor::McpSpec;

/// What an extension adds to the engine.
///
/// Only `metadata` is required; every contribution defaults to nothing. The
/// engine holds extensions as `dyn EngineExtension` in an
/// [`ExtensionSet`](super::ExtensionSet) and asks only the ones the instance
/// enabled (and the required ones). Tools, agents, commands and MCP servers
/// are read when the engine re-reads its inventory; prompt sections once per
/// step. The engine re-reads an admitted extension's contributions when the
/// Governor's generation, the enablement or that extension's own
/// `generation` changes: an extension whose contributions follow state of
/// its own (a probe that settles later) returns a value that changes with
/// that state. `generation` is compared for equality only, is called on every
/// governed moment, and must be cheap and free of IO. The Governor stays the
/// sole authority: an observer only hears, a reviewer only denies what the
/// Governor allowed, an augmenter only adds headers the Governor did not set.
pub trait EngineExtension: Send + Sync {
    fn metadata(&self) -> ExtensionMetadata;

    fn generation(&self) -> u64 {
        0
    }

    fn tools(&self) -> Vec<ExtensionTool> {
        Vec::new()
    }

    fn prompt_sections(&self, _step: &PromptContext<'_>) -> Vec<PromptSection> {
        Vec::new()
    }

    fn agents(&self, _paths: &Paths) -> Vec<AgentDef> {
        Vec::new()
    }

    fn commands(&self) -> Vec<Command> {
        Vec::new()
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        Vec::new()
    }

    fn observer(&self) -> Option<Arc<dyn GovernorObserver>> {
        None
    }

    fn reviewer(&self) -> Option<Arc<dyn ToolReviewer>> {
        None
    }

    fn augmenter(&self) -> Option<Arc<dyn CallAugmenter>> {
        None
    }
}
