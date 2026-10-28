//! Shared document store models.
//!
//! The format, projection and edit-op vocabulary is defined beside the
//! codec seam in `systemprompt_provider_contracts::documents` and
//! re-exported here, so every layer names one set of types. This module
//! adds what only the store and its clients need: access levels, actions,
//! grant audit events and access request states ([`access`]), the bridge
//! drive sync protocol ([`drive`]), the AG-UI custom event payloads
//! ([`events`], with the comment edit, delete and mention payloads in
//! [`comment_events`]) and the `document://` URI ([`uri`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod access;
pub mod awareness;
pub mod collab_events;
pub mod comment_events;
pub mod drive;
pub mod events;
pub mod sharing_events;
pub mod uri;

pub use access::{
    AccessLevel, AccessRequestStatus, ActorKind, AgentEditPolicy, DocumentAction, EditMode,
    GrantEventKind, ProposalStatus, RevisionOriginKind, UnknownDocumentValue,
};
pub use awareness::{
    AwarenessCursor, AwarenessMode, AwarenessState, AwarenessViewport, DocumentAwarenessEvent,
    DocumentLiveClosedEvent, DocumentLiveReadyEvent, DocumentLiveResyncEvent, LiveResyncReason,
};
pub use comment_events::{
    DocumentCommentDeletedEvent, DocumentCommentMentionedEvent, DocumentCommentUpdatedEvent,
    MENTION_EVENT_VERSION, MENTION_EXCERPT_CHARS,
};
pub use drive::{
    DRIVE_SYNC_AUDIENCE, DriveChange, DriveChangeKind, DriveChanges, DriveCommitMeta,
    DriveCommitResponse, DriveEntry, DriveManifest, DriveProposal, DriveProposals, DriveSyncToken,
    PresenceBeacon, PresenceSource, PresenceState,
};
pub use events::{
    DocumentAccessRequestedEvent, DocumentCollabDegradedEvent, DocumentCollabUpdateEvent,
    DocumentCommentCreatedEvent, DocumentCommentResolvedEvent, DocumentConflictEvent,
    DocumentEventName, DocumentLockEvent, DocumentOwnershipTransferredEvent, DocumentPresenceEvent,
    DocumentProposalEvent, DocumentRevisionCreatedEvent, DocumentSharedEvent, SHARED_EVENT_VERSION,
};
pub use systemprompt_provider_contracts::documents::{
    CellValue, ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
    QuoteTarget, UnknownDocumentFormat, content_hash, hash8,
};
pub use uri::{DocumentUri, DocumentUriError};
