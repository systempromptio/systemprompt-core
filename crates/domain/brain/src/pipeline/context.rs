//! [`BatchContext`]: what every event of one drain round shares, loaded
//! once per round: the declared kinds, topics, interests and importance
//! config, and the pipeline version they resolve to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_identifiers::{FactKindId, PipelineVersion, TopicId};
use systemprompt_models::brain::{BrainDocs, ImportanceConfig, InterestDecl, KindDecl};

use crate::error::BrainResult;
use crate::plane::{current_pipeline_version, load_declared_state};

/// Declarations shared by every event of one drain round.
#[derive(Debug, Clone)]
pub struct BatchContext {
    version: PipelineVersion,
    kinds: BTreeMap<FactKindId, KindDecl>,
    topics: HashSet<TopicId>,
    interests: Vec<InterestDecl>,
    importance: ImportanceConfig,
    loaded_at: DateTime<Utc>,
}

impl BatchContext {
    pub async fn load(conn: &mut PgConnection) -> BrainResult<Self> {
        let docs = load_declared_state(&mut *conn).await?.docs();
        let prompts = prompt_versions(&docs.kinds);
        let version = current_pipeline_version(&mut *conn, prompts).await?;
        Ok(Self::from_docs(docs, version, Utc::now()))
    }

    #[must_use]
    pub fn from_docs(docs: BrainDocs, version: PipelineVersion, now: DateTime<Utc>) -> Self {
        Self {
            version,
            kinds: docs
                .kinds
                .into_iter()
                .map(|kind| (kind.kind.clone(), kind))
                .collect(),
            topics: docs.topics.topics.into_iter().map(|t| t.id).collect(),
            interests: docs.interests,
            importance: docs.importance,
            loaded_at: now,
        }
    }

    #[must_use]
    pub const fn version(&self) -> &PipelineVersion {
        &self.version
    }

    #[must_use]
    pub fn kind(&self, id: &FactKindId) -> Option<&KindDecl> {
        self.kinds.get(id)
    }

    pub fn kinds(&self) -> impl Iterator<Item = &KindDecl> {
        self.kinds.values()
    }

    #[must_use]
    pub const fn topics(&self) -> &HashSet<TopicId> {
        &self.topics
    }

    #[must_use]
    pub fn interests(&self) -> &[InterestDecl] {
        &self.interests
    }

    #[must_use]
    pub const fn importance(&self) -> &ImportanceConfig {
        &self.importance
    }

    #[must_use]
    pub const fn loaded_at(&self) -> DateTime<Utc> {
        self.loaded_at
    }
}

fn prompt_versions(kinds: &[KindDecl]) -> BTreeMap<FactKindId, String> {
    kinds
        .iter()
        .filter_map(|kind| Some((kind.kind.clone(), kind.prompt_version()?.to_owned())))
        .collect()
}
