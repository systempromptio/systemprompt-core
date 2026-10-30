//! A fact kind declared under `services/brain/kinds/*.yaml`: its subject,
//! fields, statement template, identity and extractor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{FactKindId, TopicId};

pub const SUBJECT_FIELD: &str = "subject";

/// One declared fact kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindDecl {
    pub kind: FactKindId,
    pub subject: String,
    #[serde(default)]
    pub description: Option<String>,
    pub fields: BTreeMap<String, FieldDecl>,
    #[serde(default)]
    pub topics: Vec<TopicId>,
    pub statement: String,
    pub identity: Vec<String>,
    #[serde(default)]
    pub materiality: Materiality,
    #[serde(default)]
    pub review_after_days: Option<u32>,
    #[serde(default)]
    pub extractor: Option<ExtractorDecl>,
}

impl KindDecl {
    #[must_use]
    pub fn knows(&self, name: &str) -> bool {
        name == SUBJECT_FIELD || self.fields.contains_key(name)
    }

    #[must_use]
    pub fn prompt_version(&self) -> Option<&str> {
        match &self.extractor {
            Some(ExtractorDecl::Model { prompt_version, .. }) => prompt_version.as_deref(),
            _ => None,
        }
    }
}

/// The value type of a declared field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Entity,
    Date,
    Number,
    Bool,
    Enum,
}

/// One declared field of a kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDecl {
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub max_len: Option<u32>,
    #[serde(default, rename = "ref")]
    pub entity_ref: Option<String>,
    #[serde(default)]
    pub values: Vec<String>,
}

/// The fields a candidate must carry before it is worth keeping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Materiality {
    pub require: Vec<String>,
}

/// How candidates of a kind are produced: a named parser, or a model call
/// with a versioned prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtractorDecl {
    Parser {
        name: String,
    },
    Model {
        prompt: String,
        #[serde(default)]
        prompt_version: Option<String>,
    },
}
