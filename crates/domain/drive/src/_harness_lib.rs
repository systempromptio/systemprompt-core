//! The Drive: an instance's documents and folders, and a local folder kept in
//! sync with them.
//!
//! - [`client`]: [`DriveClient`], one account's document routes under `/api/v1`
//!   (listings, folders, documents and their renderings, revisions, outlines,
//!   structured edits, proposals, access, comments, creation, imports and
//!   resumable uploads), called with the account's JWT through
//!   [`sp_client::with_jwt`].
//! - [`error`]: [`DriveError`], how the instance's answers map to failures the
//!   shell and the agent can act on.
//! - [`upload`]: the part plan of a resumable upload, progress reports and
//!   cancellation.
//! - [`view`]: what the shell draws: document rows, the folder tree, comment
//!   threads, failure kinds.
//! - [`agent`]: document operations for the model, each answering compact text
//!   plus the structured result.
//! - [`extension`]: [`DriveExtension`], the Drive as the instance-side
//!   extension `drive`: the [`DriveHandle`] the shell reaches it through, the
//!   agent tools of its engine half and the [`DriveFollower`] service that
//!   keeps the local folder sync on the applied account.
//! - [`sync`]: [`DriveSync`], the local folder mirrored with the instance over
//!   the Drive sync feed: initial pull, debounced watching, periodic polls,
//!   uploads of local edits, conflicts kept beside, a state index on disk.
//!
//! Every credential stays in memory and goes only to the account's own origin;
//! nothing here logs one or puts one in an error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent;
pub mod client;
pub mod error;
pub mod extension;
mod hash;
mod http;
pub mod sync;
pub mod upload;
pub mod view;

pub use client::{
    ContentFormat, DocumentBytes, DriveClient, EditOutcome, ImportFile, ListQuery, ReadView,
    RenderedPage,
};
pub use error::{ConflictHead, DriveError, NeededCapability};
pub use extension::{
    DocumentKind, DriveEngine, DriveExtension, DriveFollower, DriveHandle, DriveSettings,
    DriveWithheld,
};
pub use sync::{
    ActivityItem, ActivityKind, DriveStatus, DriveSync, SyncAccount, SyncError, SyncOptions,
    SyncPhase, account_root, default_drive_root,
};
pub use upload::{PlannedPart, UploadProgress, plan_parts};
