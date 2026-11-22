//! `systemprompt-agent-entry` — the agent entry surface.
//!
//! The `http` feature carries the capability's conversation routes: the
//! caller's contexts, tasks and artifacts under `/api/v1/core`, and the agent
//! registry at `/api/v1/agents/registry`. The `cli` feature carries
//! `admin agents`, `core contexts` and `core artifacts`.
//! [`extension::AgentHttpExtension`] contributes both to the composition, so
//! neither the API server nor the CLI names this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
