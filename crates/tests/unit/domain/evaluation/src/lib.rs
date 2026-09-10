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

#[cfg(test)]
mod experiments;

#[cfg(test)]
mod execution_builder;

#[cfg(test)]
mod repository_runs;

#[cfg(test)]
mod repository_evidence;

#[cfg(test)]
mod repository_budget;

#[cfg(test)]
mod repository_gateway;

#[cfg(test)]
mod repository_workers;

#[cfg(test)]
mod repository_leases;

#[cfg(test)]
mod repository_events;

#[cfg(test)]
mod repository_capabilities;

#[cfg(test)]
mod repository_assignments;

#[cfg(test)]
mod experiments_execution;

#[cfg(test)]
mod experiments_builders;
