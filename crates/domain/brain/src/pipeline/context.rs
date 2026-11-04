//! [`BatchContext`]: what every event of one drain round shares.
//!
//! Loaded once per round: the declared kinds, topics, interests, importance
//! config and synonyms, and the pipeline version they resolve to. The
//! synonyms are what the round's set-based reindex expands statements
//! with, so no event and no round reads the document again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_identifiers::{FactKindId, PipelineVersion, TopicId};
use systemprompt_models::brain::{
    BrainDocs, ImportanceConfig, InterestDecl, KindDecl, SynonymsDoc,
};

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
    synonyms: SynonymsDoc,
    loaded_at: DateTime<Utc>,
}

impl BatchContext {
    pub async fn load(conn: &mut PgConnection) -> BrainResult<Self> {
        let mut declared = load_declared_state(&mut *conn).await?;
        let synonyms = declared.synonyms.take().unwrap_or_default();
        let docs = declared.docs();
        let prompts = prompt_versions(&docs.kinds);
        let version = current_pipeline_version(&mut *conn, prompts).await?;
        Ok(Self::from_docs(docs, version, Utc::now()).with_synonyms(synonyms))
    }

    #[must_use]
    pub fn with_synonyms(mut self, synonyms: SynonymsDoc) -> Self {
        self.synonyms = synonyms;
        self
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
            synonyms: SynonymsDoc::default(),
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
    pub const fn synonyms(&self) -> &SynonymsDoc {
        &self.synonyms
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
