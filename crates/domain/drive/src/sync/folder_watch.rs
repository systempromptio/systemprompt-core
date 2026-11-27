//! A debounced watch on the Drive folder that wakes the sync loop.
//!
//! An editor's save burst becomes one wake. Changes under hidden names (the
//! index, the trash, staged writes) and to ignored files never wake it. The
//! watch only hints: when it cannot start, the poll still finds every local
//! edit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use tokio::sync::Notify;

use super::names::is_ignored;

pub(crate) struct FolderWatch {
    _debouncer: Debouncer<RecommendedWatcher>,
}

impl std::fmt::Debug for FolderWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FolderWatch").finish_non_exhaustive()
    }
}

pub(crate) fn start(
    dir: &Path,
    debounce: Duration,
    wake: Arc<Notify>,
) -> Result<FolderWatch, notify_debouncer_mini::notify::Error> {
    let root = dir.to_path_buf();
    let mut debouncer = new_debouncer(debounce, move |result: DebounceEventResult| match result {
        Ok(events) => {
            if events.iter().any(|event| relevant(&root, &event.path)) {
                wake.notify_one();
            }
        },
        Err(error) => tracing::warn!(error = %error, "the Drive folder watch reported an error"),
    })?;
    debouncer.watcher().watch(dir, RecursiveMode::Recursive)?;
    Ok(FolderWatch {
        _debouncer: debouncer,
    })
}

fn relevant(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|rel| {
        rel.components().all(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| !is_ignored(name))
        })
    })
}
