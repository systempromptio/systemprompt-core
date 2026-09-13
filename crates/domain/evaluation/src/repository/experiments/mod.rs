//! Persistence for immutable experiment inputs and atomic budget admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod assignments;
mod events;
pub use assignments::{AssignmentRepository, ExecutionAssignment, ManagedWorkspaceReference};
pub use events::{ExecutionEvent, ExecutionEventBuilder, ExecutionEventRepository, ExecutionStage};
mod budget;
mod capabilities;
pub use capabilities::{
    EXECUTION_TOKEN_PREFIX, ExecutionAccess, ExecutionCapabilityRepository, ExecutionIdentity,
    ExecutionIdentityBuilder, ExecutionPrincipal,
};
mod evidence;
mod gateway;
mod leases;
mod lifecycle;
mod revisions;
mod runs;
mod workers;

pub use budget::{BudgetRepository, ReservationAdmission};
pub use evidence::EvidenceRepository;
pub use gateway::{
    AdmissionRequest, AdmissionRequestBuilder, EvaluationTrafficClass,
    GatewayEvaluationRepository, RequestAdmission,
};
pub use leases::{ExecutionCompletion, ExecutionLease, ExecutionLeaseBuilder, TerminalOutcome};
pub use lifecycle::{
    ApprovalAuthorization, ApprovalDecision, ComparisonReport, DeterministicMeasurement,
    ExecutionAccounting, GeneratedSuggestion,
    EvaluationLifecycleRepository, ExecutionApproval, SuggestionRequest,
};
pub use revisions::RevisionRepository;
pub use runs::ExperimentRepository;
pub use workers::{WorkerCredential, WorkerRecord, WorkerRecordBuilder, WorkerRepository};

pub(super) async fn lock_owner(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &systemprompt_identifiers::UserId,
) -> crate::Result<()> {
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        format!("eval-owner:{}", owner.as_str())
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
