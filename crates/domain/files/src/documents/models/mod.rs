//! Row, request and pagination types for the document store tables.
//!
//! Identifiers are the typed wrappers from `systemprompt_identifiers`;
//! columns stored as TEXT surface as the `systemprompt_models::documents`
//! enums, plus [`LockHolderKind`], which only the lock table uses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod comment;
mod cursor;
mod document;
mod folder;
mod grant;
mod lock;
mod node_id;
mod proposal;
mod revision;
mod session;
mod sharing;

pub use comment::{CommentAnchor, CommentRange, DocumentComment, MentionGrants, NewComment};
pub use cursor::{DriveCursor, ListCursor, ListCursorError, ListingCursor};
pub use document::{
    Document, DocumentSearchHit, ListScope, ListSort, ListedDocument, NewDocument, ScopedListing,
};
pub use folder::{DocumentFolder, NewFolder};
pub use grant::{DocumentGrant, GrantTarget, NewGrant, SubjectPairs};
pub use lock::{
    DocumentLock, DocumentPresenceRow, LockHolderKind, LockOutcome, LockRequest, PresenceUpsert,
};
pub use node_id::{AckClaim, NodeIdEntry, StoredWriteAck, WriteAck};
pub use proposal::{DocumentProposal, NewProposal, ProposalReview};
pub use revision::{DocumentBlob, DocumentRevision, NewRevision, RevisionAttribution};
pub use session::{DocumentEditSession, DocumentTextIndexRow, EditSessionOpen};
pub use sharing::{
    DocumentAccessRequest, DocumentGrantEvent, GrantActor, NewAccessRequest, OwnerTransfer,
};
