//! Unit tests for systemprompt-core-agent crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/agent/src/error.rs`
//! - Test: `crates/tests/unit/domain/agent/src/error.rs`
//!
//! Tests cover:
//! - Error types (TaskError, ContextError, ArtifactError, ProtocolError,
//!   AgentError)
//! - Models (skill, context, runtime, a2a protocol, web models)
//! - Services (shared utilities, agent orchestration)

// Serialises the deliberately-corrupt skill fixture writer in
// `services::skills::skill_service` against the strict `ConfigLoader::load()`
// readers in `services::registry::registry_service`. Both scan the
// process-shared bootstrap skills dir; the strict loader rejects the malformed
// stub the lenient `list_skill_ids` test stages, so the writer must hold this
// exclusively while the bad file is on disk.
#[cfg(test)]
pub(crate) static SKILLS_FIXTURE_LOCK: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());

#[cfg(test)]
mod error;

#[cfg(test)]
mod extension;

#[cfg(test)]
mod models;

#[cfg(test)]
mod services;

#[cfg(test)]
mod state;
