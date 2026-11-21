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
mod doctor;
pub use doctor::{GitCredentialCheck, classify_git_credentials};
mod error;
pub mod git_execution;
pub mod git_sources;
mod import;
mod installation;
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
    ClientEvidence, DistributionClaim, DistributionState, DistributionStatus, InstallationReceipt,
    InstallationReceiptRequest, InstalledFile,
};
pub use provenance::{SnapshotProvenance, SourceSpec};
pub use publication::{
    CohortSelector, ComparisonEvidence, GateContext, GateVerdict, INVENTORY_REFRESH_SOURCE,
    ManagedResolution, PublicationAction, PublicationAdmission, PublicationDecision,
    PublicationGate, PublicationGateError, PublicationHistoryEntry, PublicationPolicy,
    PublicationRequest, ResourceResolutionRow,
};
pub use reconciliation::{
    ConflictDecision, ConflictResolution, ReconciliationConflict, ReconciliationRecord,
    ReconciliationRequest, ReconciliationStatus,
};
pub use repository::{
    GitCredentialSource, ManagedRepository, NewResource, NewRevision, Page, ResourceKind,
    ResourceSummary, RevisionComparison, RevisionSummary, TextCandidate,
};
pub(crate) use resolver::managed_skill_from_bundle;
pub use resolver::{
    ManagedResourceResolver, ManagedSkill, ManagedSkillResolution, ResolvedManagedResource,
};
pub use source_sync::{GitSyncRequest, GitSyncResult, WithdrawalProposal, WithdrawalStatus};
pub use systemprompt_models::managed::{
    ASSEMBLER_VERSION, AssetDigest, AssetFile, DependencyRef, FileEntry, RevisionBundle,
    RevisionBundleError, RevisionFiles, RevisionManifest,
};
pub use text::normalize_form_text;
pub use tree::{CapturedSkills, capture_skills};

pub(crate) use systemprompt_models::managed::validate_path as validate_inventory_path;
pub(crate) use tree::capture_inventory_files;


mod organization_resolver;
pub use organization_resolver::{OrganizationSkillResolver, OwnedSkill};

pub use source_sync::{
    CapturedGitSource, GitCaptureRequest, GitSourceCapture, GitSynchronizationService,
    NativeGitSourceCapture,
};
