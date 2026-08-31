//! Unit tests for the systemprompt-evaluation crate.
//!
//! Test structure mirrors the source file structure:
//! - Source: `crates/domain/evaluation/src/models/*.rs` → `src/models.rs`
//! - Source: `crates/domain/evaluation/src/repository/*.rs` →
//!   `src/repository.rs`
//! - Source: `crates/domain/evaluation/src/services/*.rs` → `src/services/`

#[cfg(test)]
mod models;

#[cfg(test)]
mod repository;

#[cfg(test)]
mod services;
