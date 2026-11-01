//! [`DeclaredBrain`]: the typed brain declarations, and their projection
//! into plane entities keyed `kind/`, `topic/`, `interest/`, `workflow/`
//! and `doc/`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde::de::DeserializeOwned;
use systemprompt_extension::prelude::{ConfigPlaneResult, DeclaredState, PlaneSnapshot};
use systemprompt_models::brain::{
    BrainDocs, FiltersDoc, ImportanceConfig, InterestDecl, KindDecl, RetrievalDoc, TopicDecl,
    TopicsDoc, WorkflowProfileDecl,
};

use super::hash::PlaneRows;
use super::invalid;
use crate::models::PlaneRow;

pub(crate) const KIND: &str = "kind";
pub(crate) const TOPIC: &str = "topic";
pub(crate) const INTEREST: &str = "interest";
pub(crate) const WORKFLOW: &str = "workflow";
pub(crate) const DOC: &str = "doc";
pub(crate) const FILTERS_DOC: &str = "filters";
pub(crate) const IMPORTANCE_DOC: &str = "importance";
pub(crate) const RETRIEVAL_DOC: &str = "retrieval";

/// Every brain declaration as typed values; a scoring document absent from
/// the files or the database is `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeclaredBrain {
    pub kinds: Vec<KindDecl>,
    pub topics: Vec<TopicDecl>,
    pub interests: Vec<InterestDecl>,
    pub workflows: Vec<WorkflowProfileDecl>,
    pub filters: Option<FiltersDoc>,
    pub importance: Option<ImportanceConfig>,
    pub retrieval: Option<RetrievalDoc>,
}

impl DeclaredBrain {
    #[must_use]
    pub fn docs(&self) -> BrainDocs {
        BrainDocs {
            topics: TopicsDoc {
                topics: self.topics.clone(),
            },
            filters: self.filters.clone().unwrap_or_default(),
            importance: self.importance.clone().unwrap_or_default(),
            retrieval: self.retrieval.clone().unwrap_or_default(),
            kinds: self.kinds.clone(),
            interests: self.interests.clone(),
            workflows: self.workflows.clone(),
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.kinds.extend(other.kinds);
        self.topics.extend(other.topics);
        self.interests.extend(other.interests);
        self.workflows.extend(other.workflows);
        self.filters = other.filters.or_else(|| self.filters.take());
        self.importance = other.importance.or_else(|| self.importance.take());
        self.retrieval = other.retrieval.or_else(|| self.retrieval.take());
    }

    pub fn entity_list(&self) -> Result<Vec<(String, String)>, serde_json::Error> {
        let mut out = Vec::new();
        for kind in &self.kinds {
            out.push(entry(KIND, kind.kind.as_str(), kind)?);
        }
        for topic in &self.topics {
            out.push(entry(TOPIC, topic.id.as_str(), topic)?);
        }
        for interest in &self.interests {
            out.push(entry(INTEREST, interest.id.as_str(), interest)?);
        }
        for workflow in &self.workflows {
            out.push(entry(WORKFLOW, workflow.id.as_str(), workflow)?);
        }
        if let Some(doc) = &self.filters {
            out.push(entry(DOC, FILTERS_DOC, doc)?);
        }
        if let Some(doc) = &self.importance {
            out.push(entry(DOC, IMPORTANCE_DOC, doc)?);
        }
        if let Some(doc) = &self.retrieval {
            out.push(entry(DOC, RETRIEVAL_DOC, doc)?);
        }
        Ok(out)
    }

    pub fn entities(&self) -> Result<PlaneSnapshot, serde_json::Error> {
        Ok(self.entity_list()?.into_iter().collect())
    }

    pub fn declared_hash(&self) -> Result<String, serde_json::Error> {
        Ok(DeclaredState::from_entries(self.entities()?).hash)
    }

    pub fn from_entities(entities: &PlaneSnapshot) -> ConfigPlaneResult<Self> {
        let mut brain = Self::default();
        for (key, value) in entities {
            let (kind, id) = key
                .split_once('/')
                .ok_or_else(|| invalid(key, &"unknown entity key"))?;
            let fail = |e: serde_json::Error| invalid(key, &e);
            match (kind, id) {
                (KIND, _) => brain.kinds.push(serde_json::from_str(value).map_err(fail)?),
                (TOPIC, _) => brain
                    .topics
                    .push(serde_json::from_str(value).map_err(fail)?),
                (INTEREST, _) => brain
                    .interests
                    .push(serde_json::from_str(value).map_err(fail)?),
                (WORKFLOW, _) => brain
                    .workflows
                    .push(serde_json::from_str(value).map_err(fail)?),
                (DOC, FILTERS_DOC) => {
                    brain.filters = Some(serde_json::from_str(value).map_err(fail)?);
                },
                (DOC, IMPORTANCE_DOC) => {
                    brain.importance = Some(serde_json::from_str(value).map_err(fail)?);
                },
                (DOC, RETRIEVAL_DOC) => {
                    brain.retrieval = Some(serde_json::from_str(value).map_err(fail)?);
                },
                _ => return Err(invalid(key, &"unknown entity kind")),
            }
        }
        Ok(brain)
    }

    pub fn from_rows(rows: &PlaneRows) -> Result<Self, serde_json::Error> {
        let mut brain = Self {
            kinds: decode_all(&rows.kinds)?,
            topics: decode_all(&rows.topics)?,
            interests: decode_all(&rows.interests)?,
            workflows: decode_all(&rows.workflows)?,
            ..Self::default()
        };
        for row in &rows.docs {
            match row.id.as_str() {
                FILTERS_DOC => brain.filters = Some(decode(row)?),
                IMPORTANCE_DOC => brain.importance = Some(decode(row)?),
                RETRIEVAL_DOC => brain.retrieval = Some(decode(row)?),
                _ => {},
            }
        }
        Ok(brain)
    }
}

fn entry<T: Serialize>(
    kind: &str,
    id: &str,
    value: &T,
) -> Result<(String, String), serde_json::Error> {
    Ok((format!("{kind}/{id}"), serde_json::to_string(value)?))
}

fn decode<T: DeserializeOwned>(row: &PlaneRow) -> Result<T, serde_json::Error> {
    T::deserialize(&row.body)
}

fn decode_all<T: DeserializeOwned>(rows: &[PlaneRow]) -> Result<Vec<T>, serde_json::Error> {
    rows.iter().map(decode).collect()
}
