//! Shared source provenance, immutable revision files and managed publication.
//!
//! Import creates revisions; only a separately approved publication activates
//! them. Asset hashes establish integrity, never access permission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assets;
mod bundle;
pub mod consumer;
pub use bundle::{ASSEMBLER_VERSION, RevisionBundle};
mod diff;
pub use diff::{ChangeKind, FileChange, diff_files};
mod content_identity;
mod error;
pub mod evaluation;
#[path = "source_git_process.rs"]
pub mod git_execution;
mod import;
mod installation;
mod manifest;
mod provenance;
mod publication;
mod reconciliation;
mod repository;
mod resolver;
mod source_sync;
mod text;
mod tree;

pub use assets::{AssetDigest, AssetFile, RevisionFiles};
pub use error::{ManagedError, Result};
pub use import::ImportedSkills;
pub use installation::{
    DistributionClaim, DistributionStatus, InstallationReceipt, InstallationReceiptRequest,
    InstalledFile, InvocationAttribution, InvocationAttributionRequest, TrafficClass,
};
pub use manifest::{DependencyRef, FileEntry, RevisionManifest};
pub use provenance::{SnapshotProvenance, SourceSpec};
pub use publication::{
    ManagedResolution, PublicationAction, PublicationDecision, PublicationHistoryEntry,
    PublicationRequest,
};
pub use reconciliation::{
    ConflictDecision, ConflictResolution, ReconciliationConflict, ReconciliationRecord,
    ReconciliationRequest,
};
pub use repository::{
    ManagedRepository, NewResource, NewRevision, ResourceKind, ResourceSummary, RevisionComparison,
    RevisionSummary, TextCandidate,
};
pub use resolver::{
    ManagedResourceResolver, ManagedSkill, ManagedSkillResolution, ResolvedManagedResource,
};
pub use source_sync::{
    GitContentVerification, GitSyncRequest, GitSyncResult, GitTreeReader, GitVerificationService,
    NativeGitTreeReader, WithdrawalProposal,
};
pub use text::normalize_form_text;
pub use tree::{CapturedSkills, capture_skills};
