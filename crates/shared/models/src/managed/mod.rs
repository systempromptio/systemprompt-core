//! Verified revision closures shared across the marketplace, evaluation and
//! scheduler domains.
//!
//! A [`RevisionBundle`] is the wire form of a managed-resource revision plus
//! every revision it depends on and the exact bytes of every file. It is
//! untrusted until [`RevisionBundle::verify`] succeeds: hashes prove
//! integrity, never authorization — ownership is checked by whichever
//! repository resolved the bundle. Canonical bytes (RFC 8785 JCS) bind
//! dependency manifests, asset bytes and executable modes without filesystem
//! reads or timestamps, so the digest is stable across processes.
//!
//! Every failure is a [`RevisionBundleError`]; domain crates map it into
//! their own error enums.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assets;
mod bundle;
mod content_identity;
mod error;
mod manifest;

pub use assets::{AssetDigest, AssetFile, RevisionFiles, validate_path};
pub use bundle::{ASSEMBLER_VERSION, MAX_BYTES, MAX_FILES, MAX_REVISIONS, RevisionBundle};
pub use error::RevisionBundleError;
pub use manifest::{DependencyRef, FileEntry, RevisionManifest, validate_key};
