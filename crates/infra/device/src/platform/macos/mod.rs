//! macOS: the Keychain through the safe `security-framework` bindings, and
//! the Unix owner-only files. Generic password items live in the user's
//! default (login) keychain, one of which holds the sealing key. No `unsafe`
//! here.
//!
//! An item's access list and partition trust the binary that created it. A
//! build with another code signature (every ad-hoc signed rebuild) is asked
//! about by the system on its first read of an existing item, so that read
//! may wait on the user once per build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod passwords;
mod sealer;
mod sealing_key;
mod status;

use std::fs::{File, Metadata};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zeroize::Zeroizing;

pub use sealer::KeychainSealer;

use crate::platform::dirs::{DirsError, Environment};
use crate::platform::{CredentialStore, Platform, unix};
use crate::secure::{Sealer, SecureError};

const MACHINE_DATA: &str = "/Library/Application Support";

#[derive(Debug)]
struct LoginKeychain;

impl CredentialStore for LoginKeychain {
    fn name(&self) -> &'static str {
        "login-keychain"
    }

    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecureError> {
        passwords::store_password(service, account, secret)
    }

    fn load(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, SecureError> {
        passwords::load_password(service, account)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecureError> {
        passwords::delete_password(service, account)
    }
}

pub(crate) struct MacOs;

impl Platform for MacOs {
    fn sealer(app_id: &str) -> Arc<dyn Sealer> {
        Arc::new(KeychainSealer::new(app_id))
    }

    fn credential_store() -> Result<Arc<dyn CredentialStore>, SecureError> {
        passwords::default_keychain()?;
        Ok(Arc::new(LoginKeychain))
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

    fn local_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
        Ok(environment
            .home()?
            .join("Library")
            .join("Application Support"))
    }

    fn machine_data_dir(_environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
        Ok(PathBuf::from(MACHINE_DATA))
    }
}
