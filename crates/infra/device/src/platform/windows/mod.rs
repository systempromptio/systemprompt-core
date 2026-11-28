//! Windows: DPAPI, the Credential Manager, owner-only security descriptors
//! and the replacing rename. Its FFI modules are the only ones of the crate
//! with `unsafe`; every block states why it is sound.
//!
//! A private file carries a protected DACL granting full access to the current
//! user, SYSTEM and Administrators, nothing inherited. A private directory
//! carries the same entries marked `OICI`: Windows propagates a protected
//! directory DACL to every unprotected child, and a directory DACL without
//! inheritable entries would leave such children with an empty DACL that denies
//! even their owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod acl;
mod dpapi;
mod ffi;
mod filesystem;
mod identity;
mod sealer;

use std::fs::{self, File, Metadata};
use std::io;
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

pub use sealer::DpapiSealer;

use crate::platform::dirs::{DirsError, Environment};
use crate::platform::keyring::{KeyringStore, map_keyring};
use crate::platform::{CredentialStore, Platform};
use crate::secure::{Sealer, SecureError};

const LOCAL_DATA: &str = "LOCALAPPDATA";
const MACHINE_DATA: &str = "ProgramData";

pub(crate) struct Windows;

impl Platform for Windows {
    fn sealer(app_id: &str) -> Arc<dyn Sealer> {
        Arc::new(DpapiSealer::new(app_id.as_bytes()))
    }

    fn credential_store() -> Result<Arc<dyn CredentialStore>, SecureError> {
        let store: Arc<keyring_core::CredentialStore> =
            windows_native_keyring_store::Store::new().map_err(map_keyring)?;
        Ok(Arc::new(KeyringStore::new("credential-manager", store)))
    }

    fn is_link(metadata: &Metadata) -> bool {
        filesystem::is_reparse_point(metadata) || metadata.file_type().is_symlink()
    }

    fn create_owner_only(path: &Path) -> Result<File, SecureError> {
        let owner = identity::current_sid().map_err(|source| SecureError::Acl {
            path: path.to_path_buf(),
            source,
        })?;
        acl::create_private(path, &owner)
            .map_err(|source| SecureError::io("create private temporary file", path, source))
    }

    // Why: without FILE_FLAG_OPEN_REPARSE_POINT Windows opens what a reparse
    // point leads to; with it the point itself is opened and can be refused.
    fn open_unfollowed(path: &Path) -> Result<Option<File>, SecureError> {
        let opened = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path);
        let file = match opened {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(SecureError::io("read", path, source)),
        };
        let metadata = file
            .metadata()
            .map_err(|source| SecureError::io("inspect", path, source))?;
        if Self::is_link(&metadata) {
            return Err(SecureError::Link {
                path: path.to_path_buf(),
            });
        }
        Ok(Some(file))
    }

    // Why: Windows refuses to replace a file another handle holds open without
    // FILE_SHARE_DELETE (an antivirus scan, the search indexer), and those handles
    // close within moments. MOVEFILE_WRITE_THROUGH returns only once the rename
    // is on disk, so no directory flush follows.
    fn replace(staged: &Path, path: &Path) -> Result<(), SecureError> {
        const BACKOFF_MS: [u64; 5] = [50, 80, 120, 160, 200];
        for delay in BACKOFF_MS {
            match filesystem::move_replace(staged, path) {
                Ok(()) => return Ok(()),
                Err(e) if filesystem::is_transient_replace_error(&e) => {
                    tracing::debug!(
                        error = %e,
                        path = %path.display(),
                        delay_ms = delay,
                        "replace refused; retrying"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                },
                Err(source) => return Err(SecureError::io("replace", path, source)),
            }
        }
        filesystem::move_replace(staged, path)
            .map_err(|source| SecureError::io("replace after retries", path, source))
    }

    fn verify_owner_only(path: &Path) -> Result<(), SecureError> {
        let acl_error = |source: io::Error| SecureError::Acl {
            path: path.to_path_buf(),
            source,
        };
        let owner = identity::current_sid().map_err(acl_error)?;
        let file = File::open(path).map_err(|source| SecureError::io("open", path, source))?;
        acl::verify_private(&file, &owner).map_err(acl_error)
    }

    fn restrict_directory(path: &Path) -> Result<(), SecureError> {
        acl::protect_directory(path).map_err(|source| SecureError::Acl {
            path: path.to_path_buf(),
            source,
        })
    }

    fn local_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
        environment.directory(LOCAL_DATA)
    }

    fn machine_data_dir(environment: &Environment<'_>) -> Result<PathBuf, DirsError> {
        environment.directory(MACHINE_DATA)
    }
}
