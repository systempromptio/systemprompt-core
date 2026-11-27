//! Everything the engine reads from disk about how to behave.
//!
//! Every location is injected through [`Paths`] and [`Layout`]; nothing here
//! reads environment variables or asks the OS for well-known directories.
//! [`Loaded::load`] gathers settings, agents, skills, slash commands and
//! instruction files for one working directory, and [`Loaded::is_stale`] tells
//! a caller when to load again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agent;
mod command;
mod error;
mod frontmatter;
mod fsio;
mod instruction;
mod jsonc;
mod paths;
pub mod permission;
mod settings;
mod skill;
mod snapshot;
mod worktree;

pub use agent::{AgentDef, AgentMode, AgentOverride, Origin, ToolFilter};
pub use command::{Command, expand_template, split_arguments};
pub use error::{ConfigError, FieldError};
pub use fsio::Fingerprint;
pub use instruction::Instruction;
pub use jsonc::update_global_setting;
pub use paths::{Layout, Paths};
pub use permission::{Action, InvalidRule, Rule, Ruleset};
pub use settings::{Compaction, Formatter, LspServer, McpServer, Settings};
pub use skill::Skill;
pub use snapshot::Loaded;
pub use worktree::Worktree;
