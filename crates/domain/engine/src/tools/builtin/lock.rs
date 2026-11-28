//! Per-file locks for the tools that read, change and write a file: a step's
//! tool calls run together, so two changes to one file would otherwise each
//! read the old text and the later write would drop the earlier change.
//!
//! A lock is keyed by the file's settled, comparable path, so two spellings of
//! one file share it. Several paths are locked in sorted order, so two calls
//! that need overlapping sets never wait on each other in a cycle.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Weak};

use parking_lot::Mutex;
use tokio::sync::{Mutex as FileLock, OwnedMutexGuard};

use crate::tools::path::settle;
use crate::tools::platform;

static LOCKS: LazyLock<Mutex<HashMap<PathBuf, Weak<FileLock<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
pub(super) struct FilesLocked {
    _held: Vec<OwnedMutexGuard<()>>,
}

pub(super) async fn lock_files<'a>(paths: impl IntoIterator<Item = &'a Path>) -> FilesLocked {
    let mut keys: Vec<PathBuf> = paths
        .into_iter()
        .map(|path| platform::comparable(settle(path)))
        .collect();
    keys.sort();
    keys.dedup();
    let locks: Vec<Arc<FileLock<()>>> = {
        let mut table = LOCKS.lock();
        table.retain(|_, lock| lock.strong_count() > 0);
        keys.into_iter()
            .map(|key| {
                if let Some(lock) = table.get(&key).and_then(Weak::upgrade) {
                    return lock;
                }
                let lock = Arc::new(FileLock::new(()));
                table.insert(key, Arc::downgrade(&lock));
                lock
            })
            .collect()
    };
    let mut held = Vec::with_capacity(locks.len());
    for lock in locks {
        held.push(lock.lock_owned().await);
    }
    FilesLocked { _held: held }
}
