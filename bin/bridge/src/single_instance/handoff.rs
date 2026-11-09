//! Handing the singleton lock from a restarting bridge to its successor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use super::{SingletonGuard, SingletonResult, lock_path, try_acquire_gui};

fn handoff_path() -> PathBuf {
    lock_path().with_extension("handoff")
}

const HANDOFF_FRESH: Duration = Duration::from_secs(60);

const HANDOFF_WAIT: Duration = Duration::from_secs(20);

// Why: a restarting bridge and its successor are briefly both alive; without
// the note the successor loses the race for the singleton lock and the port.
pub(crate) fn record_handoff() {
    let path = handoff_path();
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        crate::stdio::diag(&format!("restart handoff dir {}: {e}", parent.display()));
        return;
    }
    if let Err(e) = fs::write(&path, std::process::id().to_string()) {
        crate::stdio::diag(&format!("restart handoff {}: {e}", path.display()));
    }
}

pub(crate) fn await_predecessor_exit() -> bool {
    let path = handoff_path();
    let Ok(meta) = fs::metadata(&path) else {
        return false;
    };
    let stale = meta
        .modified()
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age > HANDOFF_FRESH);
    if let Err(e) = fs::remove_file(&path) {
        crate::stdio::diag(&format!("clear restart handoff {}: {e}", path.display()));
    }
    if stale {
        return false;
    }
    if wait_for_lock().is_none() {
        crate::stdio::diag(
            "restart: the previous instance still holds the singleton lock; starting anyway",
        );
    }
    true
}

pub(super) fn wait_for_lock() -> Option<SingletonGuard> {
    let started = std::time::Instant::now();
    while started.elapsed() < HANDOFF_WAIT {
        if let SingletonResult::Acquired(guard) = try_acquire_gui() {
            return Some(guard);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}
