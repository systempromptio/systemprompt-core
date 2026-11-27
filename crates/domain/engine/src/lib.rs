//! `systemprompt-engine` — the native agent engine.
//!
//! The agent loop, tools, MCP, LSP, the workspace, the store, configuration,
//! prompts and the model transport, one top-level module each.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod config;
pub mod llm;
pub mod lsp;
pub mod mcp;
pub mod prompt;
pub mod run;
pub mod store;
pub mod tools;
pub mod workspace;
