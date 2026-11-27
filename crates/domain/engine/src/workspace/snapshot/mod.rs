//! A content-addressed shadow store per worktree.
//!
//! Each snapshot is a manifest (worktree-relative key -> content hash, size,
//! executable bit) and every file body is stored once under its SHA-256. The
//! store lives under the injected data directory, never in the user's
//! repository. Walking honours ignore files, skips symlinks and files over a
//! size cap, and reuses hashes for files whose size and mtime have not moved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod changes;
mod error;
mod fsops;
mod id;
mod layout;
mod model;
mod objects;
mod prune;
mod restore;
mod store;
mod tree;

pub use error::SnapshotError;
pub(crate) use error::io_at;
pub use id::SnapshotId;
pub use model::{DiffTarget, PrunePolicy, PruneReport, SnapshotInfo, SnapshotOptions};
pub use store::SnapshotStore;
