//! Shared source provenance, immutable revision files and managed publication.
//!
//! Import creates revisions; only a separately approved publication activates
//! them. Asset hashes establish integrity, never access permission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bundle;
pub mod consumer;
mod diff;
pub use diff::{ChangeKind, FileChange, diff_files};
mod error;
pub mod evaluation;
pub mod git_execution;
mod import;
mod installation;
mod ownership;
mod provenance;
mod publication;
mod reconciliation;
mod repository;
mod resolver;
mod source_sync;
mod text;
mod tree;

pub use error::{ManagedError, Result};
pub use import::ImportedSkills;
pub use installation::{
    DistributionClaim, DistributionStatus, InstallationReceipt, InstallationReceiptRequest,
    InstalledFile, InvocationAttribution, InvocationAttributionRequest, TrafficClass,
};
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
    GitContentVerification, GitSourceBinding, GitSyncRequest, GitSyncResult, GitTreeRead,
    GitTreeReader, GitVerificationService, NativeGitTreeReader, WithdrawalProposal,
};
pub use systemprompt_models::managed::{
    ASSEMBLER_VERSION, AssetDigest, AssetFile, DependencyRef, FileEntry, RevisionBundle,
    RevisionBundleError, RevisionFiles, RevisionManifest,
};
pub use text::normalize_form_text;
pub use tree::{CapturedSkills, capture_skills};

pub(crate) use systemprompt_models::managed::validate_path as validate_inventory_path;
pub(crate) use tree::capture_inventory_files;

pub mod operations;

mod organization_resolver;
pub use organization_resolver::OrganizationSkillResolver;

pub use source_sync::{
    CapturedGitSource, GitCaptureRequest, GitSourceCapture, GitSynchronizationService,
};
