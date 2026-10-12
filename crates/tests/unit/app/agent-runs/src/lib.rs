//! Unit tests for systemprompt-agent-runs crate
//!
//! Tests cover:
//! - RunEventPayload serde round-trips through the tagged `kind` form
//! - HostKind, RunStatus and ErrorClass text round-trips

#![allow(clippy::all)]

#[cfg(test)]
mod event_serde;
#[cfg(test)]
mod text_enums;
#[cfg(test)]
mod claude_code;
#[cfg(test)]
mod credentials;
#[cfg(test)]
mod governance;
#[cfg(test)]
mod execute;
#[cfg(test)]
mod repository;
#[cfg(test)]
mod lease;
#[cfg(test)]
mod codex_cli;
#[cfg(test)]
mod execute_support;
#[cfg(test)]
mod codex_cli_outcome;
