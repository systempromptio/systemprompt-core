//! Plain-data structs describing local-sync diffs and on-disk
//! representations of content.

mod local_sync;

pub use local_sync::{
    ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent, LocalSyncDirection,
    LocalSyncResult,
};
