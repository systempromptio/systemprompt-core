//! The brain's MCP tools for the runtime.
//!
//! The brain itself is assembled by the `brain` member when the kernel
//! builds its slot (`systemprompt_brain::BrainHandle`). [`mcp`] holds the
//! MCP tools an agent inspects team memory and proposes facts through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod mcp;

pub use systemprompt_files::documents::BRAIN_ACTOR;
