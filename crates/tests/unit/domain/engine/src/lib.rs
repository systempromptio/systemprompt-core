//! Unit tests for the systemprompt-engine crate, through its public API.
//!
//! Test structure mirrors the source module structure:
//! - Source: `crates/domain/engine/src/config/`
//! - Test: `crates/tests/unit/domain/engine/src/config/`

#[cfg(test)]
mod config;
#[cfg(test)]
mod llm;
#[cfg(test)]
mod lsp;
#[cfg(test)]
mod mcp;
#[cfg(test)]
mod prompt;
#[cfg(test)]
mod run;
#[cfg(test)]
mod store;
#[cfg(test)]
mod tools;
#[cfg(test)]
mod workspace;
