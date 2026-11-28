//! One running instance per data root, and the hand-off of a second launch's
//! arguments to it.
//!
//! The lock is a file in the data root, held for the life of the process; the
//! data root is made owner-only first, so the lock and the endpoint beside it
//! are reachable by the owner alone. The first launch becomes the primary and
//! listens on the system's local endpoint: a named pipe on Windows, a Unix
//! socket in the data root on macOS and Linux. A later launch finds the lock
//! held, sends its arguments over that endpoint and exits. The exchange is one
//! length-prefixed JSON document answered by one byte.
//!
//! [`acquire`] and both ends of the exchange run inside a Tokio runtime. A
//! target system without an endpoint does not compile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod frame;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use error::{FrameError, InstanceError};
#[cfg(target_os = "linux")]
use linux::Endpoint;
#[cfg(target_os = "macos")]
use macos::Endpoint;
#[cfg(windows)]
use windows::Endpoint;

const LOCK_FILE: &str = "instance.lock";

trait Transport: Sized {
    fn bind(data_root: &Path) -> Result<Self, InstanceError>;
    async fn receive(&mut self) -> Result<HandOff, InstanceError>;
    async fn send(data_root: &Path, hand_off: &HandOff) -> Result<(), InstanceError>;
}

/// What a second launch hands to the running instance before it exits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandOff {
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
}

/// Which launch this process is for its data root.
#[derive(Debug)]
pub enum Acquisition {
    Primary(InstanceGuard),
    Secondary(RunningInstance),
}

/// The primary's hold on the data root. Dropping it releases the lock and
/// closes the endpoint.
#[derive(Debug)]
pub struct InstanceGuard {
    endpoint: Endpoint,
    _lock: File,
}

/// The instance already running for a data root, as a later launch sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningInstance {
    data_root: PathBuf,
}

pub fn acquire(data_root: &Path) -> Result<Acquisition, InstanceError> {
    tokio::runtime::Handle::try_current().map_err(|source| InstanceError::NoRuntime { source })?;
    crate::secure::ensure_private_dir(data_root)?;
    let data_root = fs::canonicalize(data_root)
        .map_err(|source| InstanceError::io("resolve", data_root, source))?;
    let path = data_root.join(LOCK_FILE);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| InstanceError::io("open", &path, source))?;
    match lock.try_lock() {
        Ok(()) => Ok(Acquisition::Primary(InstanceGuard {
            endpoint: Endpoint::bind(&data_root)?,
            _lock: lock,
        })),
        Err(TryLockError::WouldBlock) => Ok(Acquisition::Secondary(RunningInstance { data_root })),
        Err(TryLockError::Error(source)) => Err(InstanceError::io("lock", &path, source)),
    }
}

impl InstanceGuard {
    pub async fn accept(&mut self) -> Result<HandOff, InstanceError> {
        loop {
            match self.endpoint.receive().await {
                Err(InstanceError::Rejected { source }) => {
                    tracing::warn!(error = %source, "hand-off from a second launch rejected");
                },
                other => return other,
            }
        }
    }
}

impl RunningInstance {
    #[must_use]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    pub async fn hand_off(&self, hand_off: &HandOff) -> Result<(), InstanceError> {
        Endpoint::send(&self.data_root, hand_off).await
    }
}
