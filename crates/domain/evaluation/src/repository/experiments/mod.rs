//! Persistence for immutable experiment inputs and atomic budget admission.
//!
//! Every repository is built once on the application write pool by
//! [`EvaluationRepositories::new`]; reads of rows other domains own (the AI
//! request trace, user sessions, managed revisions) arrive through the
//! shared-layer seams passed in as [`EvaluationSeams`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

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
    GatewaySeams, RequestAdmission,
};
pub use leases::{ExecutionCompletion, ExecutionLease, ExecutionLeaseBuilder, TerminalOutcome};
pub use lifecycle::{
    ApprovalAuthorization, ApprovalDecision, ApprovalVerdict, CleanupReport, ComparisonReport,
    DeterministicMeasurement, EvaluationLifecycleRepository, ExecutionAccounting,
    ExecutionApproval, GeneratedSuggestion, MeasurementRow, RetainedMeasurement, SuggestionRequest,
};
pub use revisions::RevisionRepository;
pub use runs::ExperimentRepository;
pub use workers::{WorkerCredential, WorkerRecord, WorkerRecordBuilder, WorkerRepository};

/// Shared-layer seams the evaluation repositories read foreign rows through.
#[derive(Clone)]
pub struct EvaluationSeams {
    pub trace: systemprompt_traits::DynAiRequestTrace,
    pub sessions: systemprompt_traits::DynAiSessionProvider,
    pub managed_revisions: systemprompt_traits::DynManagedRevisionOwnership,
}

impl std::fmt::Debug for EvaluationSeams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvaluationSeams").finish_non_exhaustive()
    }
}

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
    pub fn new(db: &systemprompt_database::DbPool, seams: EvaluationSeams) -> crate::Result<Self> {
        Self::with_admission(
            db,
            seams,
            Arc::new(crate::capabilities::VerifiedExecutionAdmission),
        )
    }

    pub fn with_admission(
        db: &systemprompt_database::DbPool,
        seams: EvaluationSeams,
        admission: Arc<dyn crate::capabilities::ExecutionAdmission>,
    ) -> crate::Result<Self> {
        let pool = db.write_pool_arc()?.as_ref().clone();
        let EvaluationSeams {
            trace,
            sessions,
            managed_revisions,
        } = seams;
        let budgets = BudgetRepository::new(pool.clone(), Arc::clone(&trace));
        let evidence = EvidenceRepository::new(pool.clone(), Arc::clone(&trace));
        Ok(Self {
            revisions: RevisionRepository::new(pool.clone()),
            campaigns: crate::campaigns::repository::CampaignRepository::new(
                pool.clone(),
                managed_revisions,
            ),
            assignments: AssignmentRepository::new(pool.clone(), evidence.clone()),
            capabilities: ExecutionCapabilityRepository::new(pool.clone(), Arc::clone(&sessions)),
            evidence,
            events: ExecutionEventRepository::new(pool.clone()),
            experiments: ExperimentRepository::with_admission(
                pool.clone(),
                budgets.clone(),
                Arc::clone(&admission),
            ),
            lifecycle: EvaluationLifecycleRepository::with_admission(
                pool.clone(),
                budgets.clone(),
                Arc::clone(&trace),
                Arc::clone(&admission),
            ),
            gateway: GatewayEvaluationRepository::with_admission(
                pool.clone(),
                budgets.clone(),
                GatewaySeams { trace, sessions },
                admission,
            ),
            budgets,
            workers: WorkerRepository::new(pool),
        })
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
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct CampaignAvailability {
    pub platform: String,
    pub architecture: String,
    pub admitted: bool,
    pub reason: Option<String>,
    pub variants: Vec<crate::experiments::VariantSpec>,
}

mod collections;
