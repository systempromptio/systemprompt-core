//! Repositories over the brain tables, built once into
//! [`BrainRepositories`] and shared by the pipeline, services, recall and
//! jobs.
//!
//! A write the consumer runs inside its claimed outbox transaction, or the
//! config plane inside the host's apply transaction, takes the connection as
//! an argument; reads that join no transaction run on the repository's own
//! pools. [`FactRepository::apply`] is the only path that changes a stored
//! fact, and it writes the revision row on the same connection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod disputes;
mod event_results;
mod extractions;
mod fact_apply;
mod fact_lifecycle;
mod facts;
mod failures;
mod outcomes;
mod pipeline_versions;
mod plane;
mod proposals;
mod recalls;
mod retention;
mod revisions;
mod subjects;
mod tombstones;

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

pub use disputes::DisputeRepository;
pub use event_results::EventResultRepository;
pub use extractions::ExtractionRepository;
pub use facts::{FactCounts, FactRepository};
pub use failures::FailureRepository;
pub use outcomes::OutcomeRepository;
pub use pipeline_versions::PipelineVersionRepository;
pub use plane::{
    ConfigDocRepository, InterestRepository, KindRepository, PlaneWrite, TopicRepository,
    WorkflowRepository,
};
pub use proposals::ProposalRepository;
pub use recalls::RecallRepository;
pub use retention::RetentionRepository;
pub use revisions::FactRevisionRepository;
pub use subjects::{AliasMatch, AliasRepository, SubjectRepository};
pub use tombstones::TombstoneRepository;

/// Every brain repository, constructed together from one pool.
#[derive(Debug, Clone)]
pub struct BrainRepositories {
    pub facts: Arc<FactRepository>,
    pub revisions: Arc<FactRevisionRepository>,
    pub kinds: Arc<KindRepository>,
    pub topics: Arc<TopicRepository>,
    pub interests: Arc<InterestRepository>,
    pub workflows: Arc<WorkflowRepository>,
    pub config_docs: Arc<ConfigDocRepository>,
    pub pipeline_versions: Arc<PipelineVersionRepository>,
    pub subjects: Arc<SubjectRepository>,
    pub aliases: Arc<AliasRepository>,
    pub extractions: Arc<ExtractionRepository>,
    pub event_results: Arc<EventResultRepository>,
    pub failures: Arc<FailureRepository>,
    pub disputes: Arc<DisputeRepository>,
    pub tombstones: Arc<TombstoneRepository>,
    pub recalls: Arc<RecallRepository>,
    pub outcomes: Arc<OutcomeRepository>,
    pub proposals: Arc<ProposalRepository>,
    pub retention: Arc<RetentionRepository>,
    pools: Pools,
}

impl BrainRepositories {
    pub fn new(db: &DbPool) -> Self {
        let pools = Pools::new(db);
        Self {
            facts: Arc::new(FactRepository::new(pools.clone())),
            revisions: Arc::new(FactRevisionRepository::new(pools.clone())),
            kinds: Arc::new(KindRepository::new(pools.clone())),
            topics: Arc::new(TopicRepository::new(pools.clone())),
            interests: Arc::new(InterestRepository::new(pools.clone())),
            workflows: Arc::new(WorkflowRepository::new(pools.clone())),
            config_docs: Arc::new(ConfigDocRepository::new(pools.clone())),
            pipeline_versions: Arc::new(PipelineVersionRepository::new(pools.clone())),
            subjects: Arc::new(SubjectRepository::new(pools.clone())),
            aliases: Arc::new(AliasRepository::new(pools.clone())),
            extractions: Arc::new(ExtractionRepository::new(pools.clone())),
            event_results: Arc::new(EventResultRepository::new(pools.clone())),
            failures: Arc::new(FailureRepository::new(pools.clone())),
            disputes: Arc::new(DisputeRepository::new(pools.clone())),
            tombstones: Arc::new(TombstoneRepository::new(pools.clone())),
            recalls: Arc::new(RecallRepository::new(pools.clone())),
            outcomes: Arc::new(OutcomeRepository::new(pools.clone())),
            proposals: Arc::new(ProposalRepository::new(pools.clone())),
            retention: Arc::new(RetentionRepository::new(pools.clone())),
            pools,
        }
    }

    #[must_use]
    pub fn read_pool(&self) -> &PgPool {
        &self.pools.read
    }

    #[must_use]
    pub fn write_pool(&self) -> &PgPool {
        &self.pools.write
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Pools {
    pub(crate) read: Arc<PgPool>,
    pub(crate) write: Arc<PgPool>,
}

impl Pools {
    fn new(db: &DbPool) -> Self {
        Self {
            read: db.pool(),
            write: db.write_pool(),
        }
    }
}
