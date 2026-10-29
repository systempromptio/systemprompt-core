//! Two-way sync of the user's documents with `~/<workspace>/Drive`.
//!
//! The loop in [`run`] pulls the gateway's change feed and pushes local edits
//! as document revisions. Every write into the Drive folder is a verified
//! write: the blob's SHA-256 is checked against the address it was fetched by
//! before it is staged, fsynced and renamed over the target, so a truncated or
//! substituted download never replaces a file. An upload carries the revision
//! it was based on; a gateway that has moved on answers with a conflict, and
//! the local edit is kept as a `(conflict from <host>)` sibling while the head
//! revision takes the original name. Remote deletions move the local file
//! into `.trash/<timestamp>/` rather than removing it, and a local deletion is
//! never sent: removing a document is a gateway-side act.
//!
//! [`state::DriveState`] is the only record of what was last synced. It
//! advances only after the operation it describes succeeded, so an error
//! leaves the next cycle to retry the same work. Editing presence is sent as
//! throttled beacons when a watched file changes or an agent hook touches one.
//! Each cycle also refreshes the open proposals on the user's documents, and
//! a `document.shared` or `document.proposal.opened` frame is queued as a
//! notice for the desktop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod client;
pub mod conflict;
pub mod cycle;
pub mod error;
pub mod hash;
pub mod notice;
pub mod presence;
pub mod proposals;
pub mod pull;
pub mod root;
pub mod run;
pub mod scan;
pub mod shares;
pub mod state;
pub mod status;
pub mod upload;
pub mod watcher;
pub mod write;

pub use client::DriveClient;
pub use error::DriveError;
pub use root::DriveRoot;
pub use state::{DriveEntryState, DriveState, OpenProposal, SentForReview};
pub use status::{ConflictNotice, DriveHandle, DriveStatus, EditorPresence};

use systemprompt_identifiers::DriveDeviceId;

use crate::ids::BearerToken;

/// One sync cycle's collaborators: the gateway client and bearer it runs
/// under, the folder it mirrors and the device and host names it reports.
#[derive(Debug, Clone, Copy)]
pub struct Cycle<'a> {
    pub client: &'a DriveClient,
    pub bearer: &'a BearerToken,
    pub root: &'a DriveRoot,
    pub device_id: &'a DriveDeviceId,
    pub host_name: &'a str,
}
