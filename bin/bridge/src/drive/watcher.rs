//! Filesystem watch on the Drive folder that wakes the sync loop.
//!
//! Events are debounced for [`DEBOUNCE`] so an editor's save burst becomes
//! one wake. The watcher only hints: when it cannot start, the loop's poll
//! still finds every local edit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use super::scan::is_ignored;
use super::status::DriveHandle;

pub const DEBOUNCE: Duration = Duration::from_millis(1500);

/// A running watch; dropping it stops the debouncer's thread.
pub struct DriveWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
}

impl std::fmt::Debug for DriveWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriveWatcher").finish_non_exhaustive()
    }
}

pub fn start(
    dir: &Path,
    handle: Arc<DriveHandle>,
) -> Result<DriveWatcher, notify_debouncer_mini::notify::Error> {
    let root = dir.to_path_buf();
    let mut debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| match result {
        Ok(events) => {
            let mut relevant = false;
            for event in events {
                let ignored = event.path.strip_prefix(&root).map_or(true, |rel| {
                    rel.components()
                        .any(|c| c.as_os_str().to_str().is_none_or(is_ignored))
                });
                if !ignored {
                    handle.touch(event.path);
                    relevant = true;
                }
            }
            if relevant {
                handle.sync_now();
            }
        },
        Err(error) => tracing::warn!(%error, "drive watcher reported an error"),
    })?;
    debouncer.watcher().watch(dir, RecursiveMode::Recursive)?;
    Ok(DriveWatcher {
        _debouncer: debouncer,
    })
}
