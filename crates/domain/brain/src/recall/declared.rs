//! [`DeclaredRecall`].
//!
//! The plane declarations recall and the lifecycle services read (kinds,
//! topics, workflow profiles, importance and retrieval documents), and
//! [`PlaneDeclaredSource`], which reads them from the plane tables and
//! keeps them for a short interval so a recall does not re-read every plane
//! row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_extension::brain::RecallReceipt;
use systemprompt_identifiers::{FactKindId, TopicId, WorkflowId};
use systemprompt_models::brain::{
    ImportanceConfig, KindDecl, RetrievalDoc, TopicDecl, WorkflowProfileDecl,
};
use tokio::sync::RwLock;

use crate::error::BrainResult;
use crate::repository::BrainRepositories;

const CACHE_TTL: Duration = Duration::from_secs(30);

/// The declarations recall and the lifecycle services act on; an absent
/// scoring document is its default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeclaredRecall {
    pub kinds: Vec<KindDecl>,
    pub topics: Vec<TopicDecl>,
    pub workflows: Vec<WorkflowProfileDecl>,
    pub importance: ImportanceConfig,
    pub retrieval: RetrievalDoc,
}

impl DeclaredRecall {
    #[must_use]
    pub fn kind(&self, id: &FactKindId) -> Option<&KindDecl> {
        self.kinds.iter().find(|kind| &kind.kind == id)
    }

    #[must_use]
    pub fn topic(&self, id: &TopicId) -> Option<&TopicDecl> {
        self.topics.iter().find(|topic| &topic.id == id)
    }

    #[must_use]
    pub fn workflow(&self, id: &WorkflowId) -> Option<&WorkflowProfileDecl> {
        self.workflows.iter().find(|workflow| &workflow.id == id)
    }

    #[must_use]
    pub fn workflow_named(&self, name: &str) -> Option<&WorkflowProfileDecl> {
        self.workflows
            .iter()
            .find(|workflow| workflow.id.as_str() == name)
    }
}

/// Supplies the current declarations; held as `Arc<dyn DeclaredSource>`
/// so recall and the services share one cached reader.
///
/// `#[async_trait]` keeps the trait dyn-compatible.
#[async_trait::async_trait]
pub trait DeclaredSource: Send + Sync {
    async fn load(&self) -> BrainResult<Arc<DeclaredRecall>>;
}

/// Reads the declarations from the plane tables.
#[derive(Debug)]
pub struct PlaneDeclaredSource {
    repos: BrainRepositories,
    cached: RwLock<Option<(Instant, Arc<DeclaredRecall>)>>,
}

impl PlaneDeclaredSource {
    #[must_use]
    pub const fn new(repos: BrainRepositories) -> Self {
        Self {
            repos,
            cached: RwLock::const_new(None),
        }
    }

    async fn read(&self) -> BrainResult<DeclaredRecall> {
        let mut conn = self.repos.read_pool().acquire().await?;
        let brain = crate::plane::load_declared_state(&mut conn).await?;
        Ok(DeclaredRecall {
            kinds: brain.kinds,
            topics: brain.topics,
            workflows: brain.workflows,
            importance: brain.importance.unwrap_or_default(),
            retrieval: brain.retrieval.unwrap_or_default(),
        })
    }
}

#[async_trait::async_trait]
impl DeclaredSource for PlaneDeclaredSource {
    async fn load(&self) -> BrainResult<Arc<DeclaredRecall>> {
        if let Some((at, declared)) = self.cached.read().await.as_ref()
            && at.elapsed() < CACHE_TTL
        {
            return Ok(Arc::clone(declared));
        }
        let declared = Arc::new(self.read().await?);
        *self.cached.write().await = Some((Instant::now(), Arc::clone(&declared)));
        Ok(declared)
    }
}

pub async fn receipt_workflow(
    declared: &dyn DeclaredSource,
    receipt: &RecallReceipt,
) -> Option<WorkflowId> {
    let project = receipt.principal.project.as_ref()?;
    match declared.load().await {
        Ok(declared) => declared
            .workflow_named(project)
            .map(|workflow| workflow.id.clone()),
        Err(error) => {
            tracing::warn!(
                recall_id = %receipt.recall_id,
                error = %error,
                "Brain declarations unavailable; recall recorded without its workflow"
            );
            None
        },
    }
}
