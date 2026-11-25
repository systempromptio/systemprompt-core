//! [`KernelError`]: why resolving the admitted composition against the kernel
//! failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::RuleType;
use systemprompt_extension::{DuplicateJobOwner, FactoryError, ProvideError, SlotError};
use systemprompt_governance::authz::SubjectProviderError;
use systemprompt_governance::policy::PolicyCatalogError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::DuplicateDependency;
use systemprompt_traits::SecretPattern;

#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    #[error("{owner} failed to provide {ty}")]
    Provide {
        owner: ExtensionId,
        ty: &'static str,
        #[source]
        source: ProvideError,
    },
    #[error("state value provided twice")]
    Duplicate(#[from] DuplicateDependency),
    #[error("job name contributed twice")]
    DuplicateJob(#[from] DuplicateJobOwner),
    #[error("{owner} requires {ty}, which no enabled member or the kernel provides")]
    Missing {
        owner: ExtensionId,
        ty: &'static str,
    },
    #[error(
        "{owner} confers a sealed value under row {row}, which does not name it as a core provider"
    )]
    UnlistedConferral {
        owner: ExtensionId,
        row: &'static str,
    },
    #[error("{owner} declares late cell {ty}, which only a core member may declare")]
    InstallLateCell {
        owner: ExtensionId,
        ty: &'static str,
    },
    #[error("a late cell was bound twice")]
    LateCellBound(#[from] systemprompt_models::AlreadyBound),
    #[error("{owner} provides {ty}, which only the core member {provider} provides")]
    CoreOnlyProvide {
        owner: ExtensionId,
        ty: &'static str,
        provider: &'static str,
    },
    #[error("{owner} provides {ty}, which only the kernel inserts")]
    ReservedType {
        owner: ExtensionId,
        ty: &'static str,
    },
    #[error(
        "the admitted composition was already resolved; the slot entries of {owner} were consumed"
    )]
    AlreadyResolved { owner: ExtensionId },
    #[error("slot build failed")]
    Slot(#[from] SlotError),
    #[error("user purge declarations refused")]
    Purge(#[from] systemprompt_extension::PurgeAdmissionError),
    #[error("re-seal declarations refused")]
    Reseal(#[from] systemprompt_extension::ResealAdmissionError),
    #[error("owner reassignments refused")]
    Reassignment(#[from] systemprompt_extension::ReassignmentAdmissionError),
    #[error("{owner} declares reserved subject dimension {dimension}")]
    ReservedDimension {
        owner: ExtensionId,
        dimension: RuleType,
    },
    #[error("subject dimension {dimension} declared by {second}, already owned by {first}")]
    DimensionConflict {
        dimension: RuleType,
        first: ExtensionId,
        second: ExtensionId,
    },
    #[error("subject provider factory of {owner} failed")]
    SubjectProvider {
        owner: ExtensionId,
        #[source]
        source: FactoryError,
    },
    #[error("policy catalog build failed")]
    Policies(#[from] PolicyCatalogError),
    #[error(
        "{owner} declares secret {name}, which a core member, the profile or the platform reads"
    )]
    ReservedSecret {
        owner: ExtensionId,
        name: SecretPattern,
    },
    #[error(
        "{owner} declares secret {pattern}, which no grant in the profile's extensions.secrets.{owner} covers"
    )]
    UngrantedSecret {
        owner: ExtensionId,
        pattern: SecretPattern,
    },
    #[error("{owner} declares token audience {audience}, refused: {reason}")]
    ConferredAudience {
        owner: ExtensionId,
        audience: String,
        reason: &'static str,
    },
    #[error("{owner} declares scope {scope} for token audience {audience}, refused: {reason}")]
    ConferredScope {
        owner: ExtensionId,
        audience: String,
        scope: String,
        reason: &'static str,
    },
    #[error("gateway guard factory of {owner} failed")]
    GatewayGuard {
        owner: ExtensionId,
        #[source]
        source: FactoryError,
    },
}

impl From<SubjectProviderError> for KernelError {
    fn from(error: SubjectProviderError) -> Self {
        match error {
            SubjectProviderError::Factory { owner, source } => {
                Self::SubjectProvider { owner, source }
            },
            SubjectProviderError::ReservedDimension { owner, dimension } => {
                Self::ReservedDimension { owner, dimension }
            },
            SubjectProviderError::DimensionConflict {
                dimension,
                first,
                second,
            } => Self::DimensionConflict {
                dimension,
                first,
                second,
            },
        }
    }
}
