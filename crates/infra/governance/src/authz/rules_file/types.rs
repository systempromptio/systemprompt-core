//! Wire types for `services/access-control/rules.yaml`.
//!
//! The file is entity-centric: one block per entity, `<kind>/<id>`, each
//! with a required `why`, a `default` (what a person no band names gets) and
//! `allow` / `deny` maps from a subject band to the subjects it names. A band
//! key is the database `rule_type` verbatim (`role`, or an extension slug such
//! as `group` or `project`), so the same types render a database back into
//! this shape. `user` is never a band: per-person overrides are runtime state.
//!
//! `gateway_route`, `hook` and `model` ids may be `*` globs; they are stored
//! as written and matched at decision time against the live catalog, never
//! expanded at apply. An `owner: bundle:<name>` entity arrives in a remote
//! services bundle and is applied with that bundle as its origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::authz::error::AuthzError;
use crate::authz::types::EntityKind;

pub const RULES_FILE: &str = "access-control/rules.yaml";
pub const RETIRED_ROLES_FILE: &str = "access-control/roles.yaml";
pub const OWNER_BUNDLE_PREFIX: &str = "bundle:";
pub const GLOB_KINDS: [EntityKind; 3] = [
    EntityKind::GatewayRoute,
    EntityKind::Hook,
    EntityKind::Model,
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesDoc {
    #[serde(default)]
    pub entities: Vec<EntityDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityDecl {
    pub entity: String,
    #[serde(default)]
    pub default: EntityDefault,
    pub why: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub allow: BTreeMap<String, BandSpec>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub deny: BTreeMap<String, BandSpec>,
}

impl EntityDecl {
    #[must_use]
    pub fn owner_bundle(&self) -> Option<&str> {
        self.owner.as_deref()?.strip_prefix(OWNER_BUNDLE_PREFIX)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityDefault {
    Open,
    #[default]
    Closed,
}

impl EntityDefault {
    #[must_use]
    pub const fn included(self) -> bool {
        matches!(self, Self::Open)
    }

    #[must_use]
    pub const fn from_included(included: bool) -> Self {
        if included { Self::Open } else { Self::Closed }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BandSpec {
    List(Vec<String>),
    Detailed { values: Vec<String>, why: String },
}

impl BandSpec {
    #[must_use]
    pub fn values(&self) -> &[String] {
        match self {
            Self::List(values) | Self::Detailed { values, .. } => values,
        }
    }

    #[must_use]
    pub fn why(&self) -> Option<&str> {
        match self {
            Self::List(_) => None,
            Self::Detailed { why, .. } => Some(why),
        }
    }
}

/// `<kind>/<id>` as an entity block names it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityTarget {
    pub kind: EntityKind,
    pub id: String,
}

impl EntityTarget {
    #[must_use]
    pub fn is_glob(&self) -> bool {
        self.id.contains('*')
    }
}

impl FromStr for EntityTarget {
    type Err = AuthzError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (kind, id) = text.split_once('/').ok_or_else(|| {
            AuthzError::Validation(format!("entity '{text}' must be written as <kind>/<id>"))
        })?;
        if id.trim().is_empty() || id.trim() != id {
            return Err(AuthzError::Validation(format!(
                "entity '{text}' names no id or pads it with whitespace"
            )));
        }
        Ok(Self {
            kind: EntityKind::from_str(kind)?,
            id: id.to_owned(),
        })
    }
}

impl fmt::Display for EntityTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.kind.as_str(), self.id)
    }
}
