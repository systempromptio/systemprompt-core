//! Real-repository seams for the DB-backed evaluation suites: the AI request
//! trace, the users session provider and the managed-revision ownership
//! lookup that `EvaluationSeams` carries, plus the repository constructors
//! every suite builds over them.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::{Database, DbPool};
use systemprompt_evaluation::campaigns::repository::CampaignRepository;
use systemprompt_evaluation::capabilities::ExecutionAdmission;
use systemprompt_evaluation::repository::experiments::{
    BudgetRepository, EvaluationLifecycleRepository, EvaluationRepositories, EvaluationSeams,
    EvidenceRepository, ExecutionCapabilityRepository, ExperimentRepository,
    GatewayEvaluationRepository, GatewaySeams,
};
use systemprompt_traits::{DynAiRequestTrace, DynAiSessionProvider, DynManagedRevisionOwnership};

// Every suite holds a bare write pool; the seams need the `DbPool` wrapper
// the owning repositories are constructed from.
pub fn db(pg: &PgPool) -> DbPool {
    Arc::new(Database::from_pools(Arc::new(pg.clone()), None))
}

pub fn trace(db: &DbPool) -> DynAiRequestTrace {
    seams(db).trace
}

pub fn sessions(db: &DbPool) -> DynAiSessionProvider {
    seams(db).sessions
}

pub fn revisions(db: &DbPool) -> DynManagedRevisionOwnership {
    seams(db).managed_revisions
}

pub fn seams(db: &DbPool) -> EvaluationSeams {
    systemprompt_test_fixtures::fixture_evaluation_seams(db).expect("evaluation seams")
}

pub fn repositories_with_admission(
    pg: &PgPool,
    admission: Arc<dyn ExecutionAdmission>,
) -> EvaluationRepositories {
    let db = db(pg);
    EvaluationRepositories::with_admission(&db, seams(&db), admission)
        .expect("evaluation repositories")
}

pub fn budgets(pg: &PgPool) -> BudgetRepository {
    BudgetRepository::new(pg.clone(), trace(&db(pg)))
}

pub fn capabilities(pg: &PgPool) -> ExecutionCapabilityRepository {
    ExecutionCapabilityRepository::new(pg.clone(), sessions(&db(pg)))
}

pub fn evidence(pg: &PgPool) -> EvidenceRepository {
    EvidenceRepository::new(pg.clone(), trace(&db(pg)))
}

pub fn campaigns(pg: &PgPool) -> CampaignRepository {
    CampaignRepository::new(pg.clone(), revisions(&db(pg)))
}

pub fn experiments(pg: &PgPool, admission: Arc<dyn ExecutionAdmission>) -> ExperimentRepository {
    ExperimentRepository::with_admission(pg.clone(), budgets(pg), admission)
}

pub fn lifecycle(
    pg: &PgPool,
    admission: Arc<dyn ExecutionAdmission>,
) -> EvaluationLifecycleRepository {
    EvaluationLifecycleRepository::with_admission(
        pg.clone(),
        budgets(pg),
        trace(&db(pg)),
        admission,
    )
}

pub fn gateway(pg: &PgPool, admission: Arc<dyn ExecutionAdmission>) -> GatewayEvaluationRepository {
    let db = db(pg);
    GatewayEvaluationRepository::with_admission(
        pg.clone(),
        budgets(pg),
        GatewaySeams {
            trace: trace(&db),
            sessions: sessions(&db),
        },
        admission,
    )
}

pub fn verified_admission() -> Arc<dyn ExecutionAdmission> {
    Arc::new(systemprompt_evaluation::capabilities::VerifiedExecutionAdmission)
}
