//! End-to-end lifecycle coverage for the MCP orchestrator's process layer
//! and event bus. Unit tests in `systemprompt-mcp-tests` exercise pure logic;
//! these tests drive **real** OS processes and **real** broadcast bus state.
//!
//! The fixture model is deliberately minimal: a long-running `sleep` child
//! stands in for an MCP server (the orchestrator's [`ProcessService`] cares
//! only about PID liveness, not protocol behaviour), and a tiny in-process
//! TCP listener stands in for the bound-port scenarios. This keeps the
//! integration boundary at the OS layer, which is exactly where the
//! defects this campaign is hunting (zombie leaks, stale-PID handling,
//! FD growth) live.
//!
//! Tests are POSIX-only (`#[cfg(unix)]`). Windows process handling is a
//! separate code path with its own gaps; covering it would need a
//! `tasklist`-based fixture and is filed in
//! `internal/reports/testing/findings-2026-05-25.md` if needed.
//!
//! [`ProcessService`]: systemprompt_mcp::services::process::ProcessService

#![cfg(unix)]

#[cfg(test)]
mod common;

#[cfg(test)]
mod fd_handling;
#[cfg(test)]
mod health_check_circuit_breaker;
#[cfg(test)]
mod orphaned_children;
#[cfg(test)]
mod port_binding;
#[cfg(test)]
mod session_handler;
#[cfg(test)]
mod stale_pid_cleanup;
#[cfg(test)]
mod startup_failure;
#[cfg(test)]
mod zombie_cleanup;
