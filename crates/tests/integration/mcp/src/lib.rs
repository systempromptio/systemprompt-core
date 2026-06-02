//! End-to-end lifecycle coverage for the MCP process layer and event bus,
//! driving real OS processes and a real broadcast bus. POSIX-only.

#![cfg(unix)]

#[cfg(test)]
mod common;
#[cfg(test)]
mod mock_server;

#[cfg(test)]
mod client_live;
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
