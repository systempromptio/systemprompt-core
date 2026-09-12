//! Shared source provenance, immutable revision files and managed publication.
//!
//! Import creates revisions; only a separately approved publication activates
//! them. Asset hashes establish integrity, never access permission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assets;
mod bundle;
pub use bundle::RevisionBundle;
mod diff;
pub use diff::{ChangeKind, FileChange, diff_files};
mod error;
mod import;
mod manifest;
mod provenance;
mod repository;
mod text;
mod tree;

pub use assets::{AssetDigest, AssetFile, RevisionFiles};
pub use error::{ManagedError, Result};
pub use import::ImportedSkills;
pub use manifest::{DependencyRef, FileEntry, RevisionManifest};
pub use provenance::{SnapshotProvenance, SourceSpec};
pub use repository::{
    ManagedRepository, NewResource, NewRevision, ResourceKind, ResourceSummary, RevisionComparison,
    RevisionSummary, TextCandidate,
};
pub use text::normalize_form_text;
pub use tree::{CapturedSkills, capture_skills};
