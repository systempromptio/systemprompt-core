//! [`DeclaredRecall`]: the plane declarations recall and the lifecycle
//! services read (kinds, topics, workflow profiles, importance and
//! retrieval documents), and [`PlaneDeclaredSource`], which reads them from
//! the plane tables and keeps them for a short interval so a recall does not
//! re-read every plane row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_identifiers::{FactKindId, TopicId, WorkflowId};
use systemprompt_models::brain::{
    ImportanceConfig, KindDecl, RetrievalDoc, TopicDecl, WorkflowProfileDecl,
};
use tokio::sync::RwLock;

use crate::error::BrainResult;
use crate::models::PlaneRow;
use crate::repository::{
    BrainRepositories, ConfigDocRepository, KindRepository, TopicRepository, WorkflowRepository,
};

const IMPORTANCE_DOC: &str = "importance";
const RETRIEVAL_DOC: &str = "retrieval";
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

    fn from_rows(rows: PlaneRows) -> serde_json::Result<Self> {
        let mut declared = Self {
            kinds: decode_all(rows.kinds)?,
            topics: decode_all(rows.topics)?,
            workflows: decode_all(rows.workflows)?,
            ..Self::default()
        };
        for row in rows.docs {
            match row.id.as_str() {
                IMPORTANCE_DOC => declared.importance = serde_json::from_value(row.body)?,
                RETRIEVAL_DOC => declared.retrieval = serde_json::from_value(row.body)?,
                _ => {},
            }
        }
        Ok(declared)
    }
}

struct PlaneRows {
    kinds: Vec<PlaneRow>,
    topics: Vec<PlaneRow>,
    workflows: Vec<PlaneRow>,
    docs: Vec<PlaneRow>,
}

fn decode_all<T: serde::de::DeserializeOwned>(rows: Vec<PlaneRow>) -> serde_json::Result<Vec<T>> {
    rows.into_iter()
        .map(|row| serde_json::from_value(row.body))
        .collect()
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
        let rows = PlaneRows {
            kinds: KindRepository::list(&mut *conn).await?,
            topics: TopicRepository::list(&mut *conn).await?,
            workflows: WorkflowRepository::list(&mut *conn).await?,
            docs: ConfigDocRepository::list(&mut *conn).await?,
        };
        Ok(DeclaredRecall::from_rows(rows)?)
    }
}

#[async_trait::async_trait]
impl DeclaredSource for PlaneDeclaredSource {
    async fn load(&self) -> BrainResult<Arc<DeclaredRecall>> {
        if let Some((at, declared)) = self.cached.read().await.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return Ok(Arc::clone(declared));
            }
        }
        let declared = Arc::new(self.read().await?);
        *self.cached.write().await = Some((Instant::now(), Arc::clone(&declared)));
        Ok(declared)
    }
}
