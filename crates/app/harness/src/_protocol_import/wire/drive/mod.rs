//! The Drive: documents, folders, uploads, comments, revisions, structured
//! edits and the sync feed (PROTOCOL section 12).
//!
//! The user routes under `/api/v1` take the bridge JWT as bearer; the sync feed
//! under `/v1/bridge/drive` takes a short sync token traded for the device
//! token. A key the instance omits and one it sends as `null` mean the same.
//! Timestamps on the user routes are kept as the instance's strings: the Drive
//! only displays and compares them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;
mod comments;
mod documents;
mod edits;
mod errors;
mod feed;
mod uploads;

pub use access::{
    AccessCap, CapReading, CapSource, DriveAccess, DriveAgentCap, DriveGrant, DriveGrantMade,
    InheritedFrom, ShareRequest,
};
pub use comments::{
    Attention, CommentRequest, DriveAnchor, DriveComment, DriveComments, DriveOutline, DrivePeople,
    DrivePerson, MentionOption, OutlineLine, parse_outline_line,
};
pub use documents::{
    CreateDocumentRequest, CreateFolderRequest, DriveContext, DriveContextData, DriveCreated,
    DriveDetail, DriveDocument, DriveFolder, DriveListing, DriveRevision, DriveRevisions,
    DriveScope,
};
pub use edits::{DriveLanded, DriveProposal, DriveProposals, DriveProposed, EditMode, EditRequest};
pub use errors::{DriveConflictDetail, DriveErrorBody};
pub use feed::{
    ChangeKind, CommitMeta, CommitOutcome, FeedChange, FeedChanges, FeedEntry, FeedManifest,
    MoveRequest, SyncTokenAnswer, TrashOutcome, TrashRequest,
};
pub use uploads::{
    DriveBlob, DriveUpload, DriveUploadPart, INLINE_UPLOAD_LIMIT, ImportBlobRequest,
    OpenUploadRequest,
};
