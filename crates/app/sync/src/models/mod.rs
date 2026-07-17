//! Plain-data structs describing local-sync diffs and on-disk
//! representations of content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod local_sync;

pub use local_sync::{
    ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent, LocalSyncDirection,
    LocalSyncResult,
};
