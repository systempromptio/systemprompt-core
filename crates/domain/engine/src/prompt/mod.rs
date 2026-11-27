//! Every piece of text the engine shows a model, and the assembly of the system
//! prompt.
//!
//! The crate is pure: constants and functions over borrowed input, no IO and no
//! state. The engine decides which agent is active and what the session holds;
//! this crate turns that into words.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// The built-in agents and the modes they run in.
pub mod agents;
/// The core prompt and the prompts of the built-in agents.
pub mod base;
/// The prompts that condense a long session into a summary.
pub mod compaction;
/// Descriptions and input schemas of the `task`, `skill` and `question` tools.
pub mod engine_tools;
/// The title and change-summary prompts of the hidden utility agents.
pub mod generate;
/// Synthetic messages the engine inserts into a conversation.
pub mod notices;
/// Guidance for the shell the shell tool runs.
pub mod shell;
/// Assembly of the ordered system blocks for one model call.
pub mod system;
/// Names of the tools the prompts refer to.
pub mod tool_names;

pub use agents::{AgentMode, BUILTIN_AGENTS, BuiltinAgent, DEFAULT_AGENT, builtin};
pub use shell::ShellKind;
pub use system::{
    Environment, ExtensionSection, InstructionFile, NamedEntry, ShellInUse, SystemInput, build,
};
