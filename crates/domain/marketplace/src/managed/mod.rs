//! Shared source provenance, immutable revision files and managed publication.
//!
//! Import creates revisions; only a separately approved publication activates
//! them. Asset hashes establish integrity, never access permission.

mod assets;
mod error;
mod manifest;
mod provenance;
mod repository;

pub use assets::{AssetDigest, AssetFile, RevisionFiles};
pub use error::{ManagedError, Result};
pub use manifest::{DependencyRef, FileEntry, RevisionManifest};
pub use provenance::{SnapshotProvenance, SourceSpec};
pub use repository::{ManagedRepository, NewResource, NewRevision, ResourceKind};
