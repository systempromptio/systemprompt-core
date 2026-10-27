//! Row, request and pagination types for the document store tables.
//!
//! Identifiers are the typed wrappers from `systemprompt_identifiers`;
//! columns stored as TEXT surface as the `systemprompt_models::documents`
//! enums, plus [`LockHolderKind`], which only the lock table uses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cursor;
mod document;
mod folder;
mod grant;
mod lock;
mod revision;
mod session;

pub use cursor::{ListCursor, ListCursorError};
pub use document::{Document, DocumentSearchHit, NewDocument, VisibleListing};
pub use folder::{DocumentFolder, NewFolder};
pub use grant::{DocumentGrant, GrantTarget, NewGrant, SubjectPairs};
pub use lock::{
    DocumentLock, DocumentPresenceRow, LockHolderKind, LockOutcome, LockRequest, PresenceUpsert,
};
pub use revision::{DocumentBlob, DocumentRevision, NewRevision};
pub use session::{DocumentEditSession, DocumentTextIndexRow, EditSessionOpen};
