//! Persistence for immutable experiment inputs and atomic budget admission.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admission;
mod assignments;
mod campaign_runs;
pub use campaign_runs::CampaignExperiment;
mod events;
mod holdout;
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
pub use evidence::{EvidenceRepository, ManagedWorkspaceRegistration};
pub use gateway::{
    AdmissionRequest, AdmissionRequestBuilder, EvaluationTrafficClass, GatewayEvaluationRepository,
    RequestAdmission,
};
pub use leases::{ExecutionCompletion, ExecutionLease, ExecutionLeaseBuilder, TerminalOutcome};
pub use lifecycle::{
    ApprovalAuthorization, ApprovalDecision, ApprovalVerdict, CleanupReport, ComparisonReport,
    DeterministicMeasurement, EvaluationLifecycleRepository, ExecutionAccounting,
    ExecutionApproval, GeneratedSuggestion, SuggestionRequest,
};
pub use revisions::RevisionRepository;
pub use runs::ExperimentRepository;
pub use workers::{WorkerCredential, WorkerRecord, WorkerRecordBuilder, WorkerRepository};

#[derive(Debug, Clone)]
pub struct EvaluationRepositories {
    pub revisions: RevisionRepository,
    pub budgets: BudgetRepository,
    pub campaigns: crate::campaigns::repository::CampaignRepository,
    pub assignments: AssignmentRepository,
    pub capabilities: ExecutionCapabilityRepository,
    pub evidence: EvidenceRepository,
    pub events: ExecutionEventRepository,
    pub experiments: ExperimentRepository,
    pub lifecycle: EvaluationLifecycleRepository,
    pub gateway: GatewayEvaluationRepository,
    pub workers: WorkerRepository,
}

impl EvaluationRepositories {
    #[must_use]
    pub fn new(pool: &sqlx::PgPool) -> Self {
        Self::with_admission(
            pool,
            std::sync::Arc::new(crate::capabilities::VerifiedExecutionAdmission),
        )
    }

    pub fn with_admission(
        pool: &sqlx::PgPool,
        admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
    ) -> Self {
        Self {
            revisions: RevisionRepository::new(pool.clone()),
            budgets: BudgetRepository::new(pool.clone()),
            campaigns: crate::campaigns::repository::CampaignRepository::new(pool.clone()),
            assignments: AssignmentRepository::new(pool.clone()),
            capabilities: ExecutionCapabilityRepository::new(pool.clone()),
            evidence: EvidenceRepository::new(pool.clone()),
            events: ExecutionEventRepository::new(pool.clone()),
            experiments: ExperimentRepository::with_admission(pool.clone(), admission.clone()),
            lifecycle: EvaluationLifecycleRepository::with_admission(
                pool.clone(),
                admission.clone(),
            ),
            gateway: GatewayEvaluationRepository::with_admission(pool.clone(), admission),
            workers: WorkerRepository::new(pool.clone()),
        }
    }
}

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

/// Current admission decision for the exact frozen execution variants.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CampaignAvailability {
    pub platform: String,
    pub architecture: String,
    pub admitted: bool,
    pub reason: Option<String>,
    pub variants: Vec<crate::experiments::VariantSpec>,
}
