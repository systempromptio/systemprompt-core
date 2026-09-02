//! Vendor-agnostic file storage for systemprompt.io.
//!
//! Every writer of user-visible files (uploads, generated images) goes
//! through the [`FileStorage`] trait so the backend can be swapped without
//! touching the domain crates. Today the only backend is
//! [`LocalFileStorage`], which writes under a single root directory; that
//! root may be a local disk or a shared mount visible to every replica.
//!
//! Storage ids are paths relative to the root (`files/uploads/…`). They are
//! validated on every call: absolute paths and `..` components are rejected
//! before any filesystem access.
//!
//! # Modules
//!
//! - [`local`] — [`LocalFileStorage`] and the id-to-path resolver.
//! - [`probe`] — [`probe_shared_mount`], the boot-time check that a
//!   `storage.shared` root really is shared between replicas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod local;
pub mod probe;

use std::path::Path;
use std::sync::Arc;

use systemprompt_models::profile::StorageBackend;
use systemprompt_traits::FileStorage;

pub use local::LocalFileStorage;
pub use probe::{SharedMountReport, probe_shared_mount};

#[must_use]
pub fn build_file_storage(backend: StorageBackend, root: &Path) -> Arc<dyn FileStorage> {
    match backend {
        StorageBackend::Local => Arc::new(LocalFileStorage::new(root.to_path_buf())),
    }
}
