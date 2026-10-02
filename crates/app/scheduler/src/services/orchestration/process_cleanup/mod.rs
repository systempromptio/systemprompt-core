//! Cross-platform process and port cleanup primitives.
//!
//! [`ProcessCleanup`] exposes a uniform API; the platform-specific
//! implementations live in `posix` (Unix) and `winnt` (Windows) and are
//! gated by `#[cfg(unix)]` / `#[cfg(windows)]`.
//!
//! A port lookup only ever reports listening sockets, and port 0 ("no port")
//! is never looked up: asking the OS for it matches unrelated sockets on the
//! host. A PID found on a port is a holder, not an identity — callers verify
//! it (spawn marker, recorded PID or [`ProcessCleanup::is_peer_instance`])
//! before signalling it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod listener;
#[cfg(unix)]
mod posix;
#[cfg(windows)]
mod winnt;

#[cfg(unix)]
use posix as platform;
#[cfg(windows)]
use winnt as platform;

use std::num::NonZeroU16;
use std::path::Path;

use crate::error::{PortHolder, SchedulerError, SchedulerResult};

const PROTECTED_PORTS: &[u16] = &[5432, 6432];
const PROTECTED_PROCESSES: &[&str] = &["postgres", "pgbouncer", "psql"];

#[derive(Debug, Clone, Copy)]
pub struct ProcessCleanup;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub port: u16,
}

impl ProcessCleanup {
    pub fn listener_pids(port: NonZeroU16) -> Vec<u32> {
        if PROTECTED_PORTS.contains(&port.get()) {
            return vec![];
        }
        platform::listener_pids(port)
    }

    pub fn check_port(port: u16) -> Option<u32> {
        let port = NonZeroU16::new(port)?;
        Self::listener_pids(port).into_iter().next()
    }

    pub fn kill_port(port: u16, owner: u32) -> Vec<u32> {
        let Some(holder) = Self::check_port(port) else {
            return vec![];
        };

        if holder != owner && platform::process_group(holder) != Some(owner) {
            tracing::warn!(
                port,
                holder,
                owner,
                "port held by a process that is not the service being stopped; leaving it untouched",
            );
            return vec![];
        }

        if Self::kill_process(holder) {
            vec![holder]
        } else {
            vec![]
        }
    }

    pub fn is_peer_instance(pid: u32) -> bool {
        if pid == std::process::id() {
            return false;
        }
        let Some(own_name) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.file_name().and_then(|n| n.to_str()).map(str::to_owned))
        else {
            tracing::warn!(
                pid,
                "cannot resolve the running executable; treating holder as foreign"
            );
            return false;
        };
        platform::process_name(pid).is_some_and(|name| {
            Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|holder| listener::executable_names_match(holder, &own_name))
        })
    }

    pub fn kill_process(pid: u32) -> bool {
        platform::kill_process(pid)
    }

    pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
        platform::terminate_gracefully(pid, grace_period_ms).await
    }

    pub async fn terminate_group_gracefully(pgid: u32, grace_period_ms: u64) -> bool {
        platform::terminate_group_gracefully(pgid, grace_period_ms).await
    }

    pub fn process_exists(pid: u32) -> bool {
        platform::process_exists(pid)
    }

    pub fn kill_by_pattern(pattern: &str) -> usize {
        if PROTECTED_PROCESSES
            .iter()
            .any(|protected| pattern.contains(protected))
        {
            return 0;
        }
        platform::kill_by_pattern(pattern)
    }

    pub async fn wait_for_port_free(
        port: u16,
        max_retries: u8,
        retry_delay_ms: u64,
    ) -> SchedulerResult<()> {
        if NonZeroU16::new(port).is_none() {
            return Ok(());
        }
        for attempt in 1..=max_retries {
            if Self::check_port(port).is_none() {
                return Ok(());
            }

            if attempt < max_retries {
                tokio::time::sleep(tokio::time::Duration::from_millis(retry_delay_ms)).await;
            }
        }

        Err(SchedulerError::PortOccupied {
            port,
            holder: Self::check_port(port).map_or(PortHolder::Unknown, PortHolder::Pid),
            attempts: max_retries,
        })
    }

    pub fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
        let pid = Self::check_port(port)?;
        let name = platform::process_name(pid)?;
        Some(ProcessInfo { pid, name, port })
    }
}
