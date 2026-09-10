//! Handing the singleton lock from a restarting bridge to its successor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use super::{SingletonResult, lock_path, try_acquire_gui};

fn handoff_path() -> PathBuf {
    lock_path().with_extension("handoff")
}

const HANDOFF_FRESH: Duration = Duration::from_secs(60);

const HANDOFF_WAIT: Duration = Duration::from_secs(20);

// Why: a restarting bridge spawns its successor and then exits, so for a moment
// both are alive and the successor would lose the race for the singleton lock
// and the default proxy port. The note tells it to wait instead. Written on a
// best-effort basis: without it the successor simply starts as it does today.
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

// Why: runs before anything binds a port or acquires the lock. A note older
// than HANDOFF_FRESH is a leftover from a crash, not a handoff, and is
// discarded without waiting.
pub(crate) fn await_predecessor_exit() {
    let path = handoff_path();
    let Ok(meta) = fs::metadata(&path) else {
        return;
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
        return;
    }
    let started = std::time::Instant::now();
    while started.elapsed() < HANDOFF_WAIT {
        if matches!(try_acquire_gui(), SingletonResult::Acquired(_)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    crate::stdio::diag(
        "restart: the previous instance still holds the singleton lock; starting anyway",
    );
}
