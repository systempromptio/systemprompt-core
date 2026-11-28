//! The contract each target system implements, and the credential store it
//! hands out.
//!
//! Every function of the platform trait is associated, not a method: the
//! implementation is chosen when the crate is compiled, never at run time. A
//! staged file is created owner-only before a byte is written to it; an
//! unfollowed open never goes through a link; a replace returns once the
//! rename is on disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zeroize::Zeroizing;

use crate::platform::dirs::{DirsError, Environment};
use crate::secure::{Sealer, SecureError};

pub(crate) trait CredentialStore: Send + Sync + fmt::Debug {
    fn name(&self) -> &'static str;

    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecureError>;

    fn load(&self, service: &str, account: &str)
    -> Result<Option<Zeroizing<Vec<u8>>>, SecureError>;

    fn delete(&self, service: &str, account: &str) -> Result<(), SecureError>;
}

pub(crate) trait Platform {
    fn sealer(app_id: &str) -> Arc<dyn Sealer>;

    fn credential_store() -> Result<Arc<dyn CredentialStore>, SecureError>;

    fn is_link(metadata: &Metadata) -> bool;

    fn create_owner_only(path: &Path) -> Result<File, SecureError>;

    fn open_unfollowed(path: &Path) -> Result<Option<File>, SecureError>;

    fn replace(staged: &Path, path: &Path) -> Result<(), SecureError>;

    fn verify_owner_only(path: &Path) -> Result<(), SecureError>;

    fn restrict_directory(path: &Path) -> Result<(), SecureError>;

    fn local_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError>;

    fn machine_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError>;
}
