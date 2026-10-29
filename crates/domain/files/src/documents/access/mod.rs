//! Document and folder authorization.
//!
//! [`DocumentAccessService::authorize`] answers one question in two steps,
//! and both must pass:
//!
//! 1. **Grants.** The owner of a document or folder holds `own`. Everyone else
//!    holds the highest unexpired grant naming one of their subject pairs
//!    (`user`, each role, and every value of every registered subject
//!    dimension) on the document, on its folder or on any ancestor folder: a
//!    folder grant covers its whole subtree. Creating a folder grants its
//!    creator `own` on it, so the subtree stays theirs after documents from
//!    others land in it. The action's required level must not exceed the level
//!    held. A grant never confers `own`: the level read from grants is capped
//!    at `share`, so `own`-gated actions (delete, trash, restore, move,
//!    transfer, revoke every grant) belong to the owner alone. Administrators
//!    are not special-cased: they reach documents through grants to their role
//!    like anyone else.
//! 2. **Policy.** The authz hook must allow an [`AuthzRequest`] whose entity is
//!    the nearest folder and whose context is `document.access`, carrying the
//!    folders above it nearest-first, so a rule on any ancestor folder governs
//!    its whole subtree, and the caller's subject attributes. Documents and
//!    folders at the root are checked against the root folder entity
//!    ([`root_folder_entity`]), so an operator can govern the root like any
//!    folder. [`DocumentAccessService::probe`] asks the same question through
//!    the hook's `probe`, which writes no audit row, for listings.
//!
//! The root is not an object and holds no grants: any authenticated user may
//! create documents and folders there (subject to the hook), and what they
//! create is visible only to its owner until it is shared.
//!
//! A caller who holds no level on an existing document or folder is told it
//! does not exist; the owner is named only to callers who hold some level.
//!
//! Deleting a folder is one decision over its whole subtree: the caller must
//! hold `delete` on the folder, on every folder below it and on every live
//! document inside it. Owning the top folder confers nothing on what a
//! collaborator owns inside it; one refusal refuses the whole delete before
//! anything is trashed, and the trash itself is a single transaction.
//!
//! Every read a decision rests on runs on the primary, never a replica, so a
//! revocation, trash or transfer applies to the next request.
//!
//! Every step fails closed: a subject provider error, an unknown target or a
//! hook deny is a refusal, never a partial answer.
//!
//! [`AuthzRequest`]: systemprompt_governance::authz::AuthzRequest
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agent;
mod agent_moves;
mod agent_queries;
mod catalog;
mod directory;
mod probe;
mod request;
mod resolve;
mod search;
mod service;
mod subjects;
mod subtree;

pub use agent_moves::MoveTarget;
pub use directory::{SubjectDirectory, SubjectDirectoryError, SubjectMatch};
pub use probe::FolderProbe;
pub use search::{MAX_SUBJECT_SEARCH, SubjectSearch};
pub use service::{
    AccessCheck, AccessTarget, DocumentAccessRepositories, DocumentAccessService, Granted,
    root_folder_entity,
};
pub use subtree::SubtreePartition;
