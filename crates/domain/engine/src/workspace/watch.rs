//! Debounced recursive file watching. Each watched directory has its own
//! debouncer; events from all of them go out on one broadcast channel after
//! ignore rules are applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify_debouncer_mini::notify::{self, RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

const ALWAYS_IGNORED: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".jj",
    "node_modules",
    "target",
    ".venv",
    "__pycache__",
    ".next",
    ".turbo",
    ".cache",
];
const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(300);
const DEFAULT_CAPACITY: usize = 1024;

/// What happened to a path, as far as can be told after debouncing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEventKind {
    Changed,
    Removed,
}

/// A debounced change under a watched directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: FileEventKind,
}

/// Debounce window, extra gitignore-style patterns and channel capacity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchOptions {
    pub debounce: Duration,
    pub ignore: Vec<String>,
    pub capacity: usize,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce: DEFAULT_DEBOUNCE,
            ignore: Vec::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }
}

/// Failures starting a watch.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("could not watch {}", path.display())]
    Watch {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
}

/// Watches any number of directories and broadcasts their changes.
pub struct FileWatcher {
    options: WatchOptions,
    sender: broadcast::Sender<FileEvent>,
    active: Mutex<HashMap<PathBuf, Debouncer<RecommendedWatcher>>>,
}

impl fmt::Debug for FileWatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileWatcher")
            .field("options", &self.options)
            .field("watching", &self.watching())
            .finish_non_exhaustive()
    }
}

impl FileWatcher {
    pub fn new(options: WatchOptions) -> Self {
        let (sender, _) = broadcast::channel(options.capacity.max(1));
        Self {
            options,
            sender,
            active: Mutex::new(HashMap::new()),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FileEvent> {
        self.sender.subscribe()
    }

    pub fn watch(&self, dir: &Path) -> Result<(), WatchError> {
        if self.active.lock().contains_key(dir) {
            return Ok(());
        }
        let filter = Arc::new(WatchFilter::new(dir, &self.options.ignore));
        let sender = self.sender.clone();
        let handler_filter = Arc::clone(&filter);
        let handler = move |result: DebounceEventResult| match result {
            Ok(events) => {
                for event in events {
                    if !handler_filter.allows(&event.path) {
                        continue;
                    }
                    let kind = if event.path.symlink_metadata().is_ok() {
                        FileEventKind::Changed
                    } else {
                        FileEventKind::Removed
                    };
                    if sender
                        .send(FileEvent {
                            path: event.path,
                            kind,
                        })
                        .is_err()
                    {
                        tracing::trace!("file event dropped: no subscribers");
                    }
                }
            },
            Err(error) => tracing::warn!(%error, "file watcher reported an error"),
        };
        let watch_error = |source: notify::Error| WatchError::Watch {
            path: dir.to_path_buf(),
            source,
        };
        let mut debouncer = new_debouncer(self.options.debounce, handler).map_err(watch_error)?;
        notify::Watcher::watch(debouncer.watcher(), dir, RecursiveMode::Recursive)
            .map_err(watch_error)?;
        tracing::debug!(dir = %dir.display(), "watching directory");
        self.active
            .lock()
            .entry(dir.to_path_buf())
            .or_insert(debouncer);
        Ok(())
    }

    pub fn unwatch(&self, dir: &Path) -> bool {
        let removed = self.active.lock().remove(dir).is_some();
        if removed {
            tracing::debug!(dir = %dir.display(), "stopped watching directory");
        }
        removed
    }

    pub fn is_watching(&self, dir: &Path) -> bool {
        self.active.lock().contains_key(dir)
    }

    pub fn watching(&self) -> Vec<PathBuf> {
        self.active.lock().keys().cloned().collect()
    }

    pub fn stop_all(&self) {
        self.active.lock().clear();
    }
}

#[derive(Debug)]
pub(crate) struct WatchFilter {
    roots: Vec<PathBuf>,
    rules: Gitignore,
}

impl WatchFilter {
    pub(crate) fn new(root: &Path, extra: &[String]) -> Self {
        let mut roots = vec![root.to_path_buf()];
        if let Ok(canonical) = root.canonicalize()
            && canonical != root
        {
            roots.push(canonical);
        }
        let mut builder = GitignoreBuilder::new(root);
        for name in [".gitignore", ".ignore"] {
            let path = root.join(name);
            if path.is_file()
                && let Some(error) = builder.add(&path)
            {
                tracing::warn!(path = %path.display(), %error, "ignoring unreadable ignore file");
            }
        }
        for pattern in extra {
            if let Err(error) = builder.add_line(None, pattern) {
                tracing::warn!(%pattern, %error, "ignoring invalid watch ignore pattern");
            }
        }
        let rules = builder.build().unwrap_or_else(|error| {
            tracing::warn!(%error, "watch ignore rules failed to build");
            Gitignore::empty()
        });
        Self { roots, rules }
    }

    pub(crate) fn allows(&self, path: &Path) -> bool {
        let Some(relative) = self
            .roots
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())
        else {
            return true;
        };
        if relative.as_os_str().is_empty() {
            return false;
        }
        let hidden_by_name = relative.components().any(|component| {
            let name = component.as_os_str();
            ALWAYS_IGNORED
                .iter()
                .any(|ignored| name.eq_ignore_ascii_case(ignored))
        });
        if hidden_by_name {
            return false;
        }
        !self
            .rules
            .matched_path_or_any_parents(relative, path.is_dir())
            .is_ignore()
    }
}
