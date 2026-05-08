//! Plain-data structs describing local-sync diffs and on-disk
//! representations of agents / content.

mod local_sync;

pub use local_sync::{
    AgentDiffItem, AgentsDiffResult, ContentDiffItem, ContentDiffResult, DiffStatus, DiskAgent,
    DiskContent, LocalSyncDirection, LocalSyncResult,
};
