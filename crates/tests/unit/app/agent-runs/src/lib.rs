//! Unit tests for systemprompt-agent-runs crate
//!
//! Tests cover:
//! - RunEventPayload serde round-trips through the tagged `kind` form
//! - HostKind, RunStatus and ErrorClass text round-trips

#![allow(clippy::all)]

#[cfg(test)]
mod a2a;
#[cfg(test)]
mod a2a_repository;
#[cfg(test)]
mod a2a_updates_extra;
#[cfg(test)]
mod claude_code;
#[cfg(test)]
mod claude_code_prepare;
#[cfg(test)]
mod codex_cli;
#[cfg(test)]
mod codex_cli_outcome;
#[cfg(test)]
mod credentials;
#[cfg(test)]
mod credentials_extra;
#[cfg(test)]
mod event_serde;
#[cfg(test)]
mod execute;
#[cfg(test)]
mod execute_support;
#[cfg(test)]
mod governance;
#[cfg(test)]
mod harness;
#[cfg(test)]
mod lease;
#[cfg(test)]
mod opencode;
#[cfg(test)]
mod opencode_outcome;
#[cfg(test)]
mod repository;
#[cfg(test)]
mod runner;
#[cfg(test)]
mod runner_claim;
#[cfg(test)]
mod task_link;
#[cfg(test)]
mod text_enums;
