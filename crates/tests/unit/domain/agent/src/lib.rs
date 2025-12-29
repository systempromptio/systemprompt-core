//! Unit tests for systemprompt-core-agent crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/agent/src/error.rs`
//! - Test: `crates/tests/unit/domain/agent/src/error.rs`
//!
//! Tests cover:
//! - Error types (TaskError, ContextError, ArtifactError, ProtocolError, AgentError)
//! - Models (skill, context, runtime, a2a protocol, web models)
//! - Services (shared utilities, agent orchestration)

#[cfg(test)]
mod error;

#[cfg(test)]
mod models;

#[cfg(test)]
mod services;
