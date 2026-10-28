//! Shared document store models.
//!
//! The format, projection and edit-op vocabulary is defined beside the
//! codec seam in `systemprompt_provider_contracts::documents` and
//! re-exported here, so every layer names one set of types. This module
//! adds what only the store and its clients need: access levels and
//! actions ([`access`]), the bridge drive sync protocol ([`drive`]), the
//! AG-UI custom event payloads ([`events`]) and the `document://` URI
//! ([`uri`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod access;
pub mod awareness;
pub mod drive;
pub mod events;
pub mod uri;

pub use access::{
    AccessLevel, ActorKind, AgentEditPolicy, DocumentAction, EditMode, ProposalStatus,
    RevisionOriginKind, UnknownDocumentValue,
};
pub use awareness::{
    AwarenessCursor, AwarenessMode, AwarenessState, AwarenessViewport, DocumentAwarenessEvent,
    DocumentLiveClosedEvent, DocumentLiveReadyEvent, DocumentLiveResyncEvent, LiveResyncReason,
};
pub use drive::{
    DriveChange, DriveChangeKind, DriveChanges, DriveCommitMeta, DriveCommitResponse, DriveEntry,
    DriveManifest, DriveProposal, DriveProposals, PresenceBeacon, PresenceSource, PresenceState,
};
pub use events::{
    DocumentCollabUpdateEvent, DocumentCommentCreatedEvent, DocumentCommentResolvedEvent,
    DocumentConflictEvent, DocumentEventName, DocumentLockEvent, DocumentOwnershipTransferredEvent,
    DocumentPresenceEvent, DocumentProposalEvent, DocumentRevisionCreatedEvent,
    DocumentSharedEvent,
};
pub use systemprompt_provider_contracts::documents::{
    CellValue, ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
    QuoteTarget, UnknownDocumentFormat, content_hash, hash8,
};
pub use uri::{DocumentUri, DocumentUriError};
