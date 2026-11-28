//! Linux: the Secret Service and the kernel keyring, the Unix owner-only
//! files, and the XDG base directories. No `unsafe` here.
//!
//! A desktop session has a Secret Service provider, which keeps a secret
//! across logins. A headless machine has none; there the kernel keyring of
//! the session holds it, between processes but not across a reboot. Where
//! neither is usable (a container whose seccomp profile refuses `add_key`)
//! the sealer keeps its key in this process's memory, so a sign-in works and
//! ends with the process; the credential store has no such third tier and
//! reports itself unavailable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod keyrings;
mod sealer;
mod sealing_key;

use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use sealer::KeyringSealer;

use crate::platform::dirs::{DirsError, Environment};
use crate::platform::{CredentialStore, Platform, unix};
use crate::secure::{Sealer, SecureError};

const DATA_HOME: &str = "XDG_DATA_HOME";
const MACHINE_DATA: &str = "/var/lib";

pub(crate) struct Linux;

impl Platform for Linux {
    fn sealer(app_id: &str) -> Arc<dyn Sealer> {
        Arc::new(KeyringSealer::new(app_id))
    }

    fn credential_store() -> Result<Arc<dyn CredentialStore>, SecureError> {
        keyrings::first_usable()
    }

    fn is_link(metadata: &Metadata) -> bool {
        unix::is_link(metadata)
    }

    fn create_owner_only(path: &Path) -> Result<File, SecureError> {
        unix::create_owner_only(path)
    }

    fn open_unfollowed(path: &Path) -> Result<Option<File>, SecureError> {
        unix::open_unfollowed(path)
    }

    fn replace(staged: &Path, path: &Path) -> Result<(), SecureError> {
        unix::replace(staged, path)
    }

    fn verify_owner_only(path: &Path) -> Result<(), SecureError> {
        unix::verify_owner_only(path)
    }

    fn restrict_directory(path: &Path) -> Result<(), SecureError> {
        unix::restrict_directory(path)
    }

    // Why: the XDG Base Directory Specification has a relative
    // `XDG_DATA_HOME` ignored and `$HOME/.local/share` used in its place.
    fn local_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
        environment
            .directory(DATA_HOME)
            .or_else(|_relative_or_unset| Ok(environment.home()?.join(".local").join("share")))
    }

    fn machine_data_dir(_environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
        Ok(PathBuf::from(MACHINE_DATA))
    }
}
