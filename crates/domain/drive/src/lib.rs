//! `systemprompt-drive` — folder sync.
//!
//! An instance's documents and folders, and a local folder kept in sync with
//! them.
//!
//! - [`client`]: [`DriveClient`], one account's document routes under `/api/v1`
//!   (listings, folders, documents and their renderings, revisions, outlines,
//!   structured edits, proposals, access, comments, creation, imports and
//!   resumable uploads), called with the account's JWT.
//! - [`error`]: [`DriveError`], how the instance's answers map to failures the
//!   shell and the agent can act on.
//! - [`upload`]: the part plan of a resumable upload, progress reports and
//!   cancellation.
//! - [`sync`]: [`DriveSync`], the local folder mirrored with the instance over
//!   the Drive sync feed: initial pull, debounced watching, periodic polls,
//!   uploads of local edits, conflicts kept beside, a state index on disk.
//!
//! The Drive's member of the device composition, with its agent tools, is in
//! `systemprompt-harness`. Every credential stays in memory and goes only to
//! the account's own origin; nothing here logs one or puts one in an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod client;
pub mod error;
mod hash;
mod http;
pub mod sync;
pub mod upload;

pub use client::{
    ContentFormat, DocumentBytes, DriveClient, EditOutcome, ImportFile, ListQuery, ReadView,
    RenderedPage,
};
pub use error::{ConflictHead, DriveError, NeededCapability};
pub use sync::{
    ActivityItem, ActivityKind, DriveStatus, DriveSync, SyncAccount, SyncError, SyncOptions,
    SyncPhase, account_root, default_drive_root,
};
pub use upload::{PlannedPart, UploadProgress, plan_parts};
