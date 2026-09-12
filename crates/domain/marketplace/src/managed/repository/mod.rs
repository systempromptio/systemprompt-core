//! Owner-scoped persistence. Content writes are immutable and idempotent.

mod revisions;
mod sources;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use systemprompt_identifiers::{
    ManagedResourceId, ManagedSourceId, ResourceRevisionId, SourceSnapshotId,
};

use super::{DependencyRef, RevisionFiles};

#[derive(Debug, Clone)]
pub struct ManagedRepository {
    pool: PgPool,
}

impl ManagedRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Skill,
    Plugin,
    Marketplace,
    Supporting,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Plugin => "plugin",
            Self::Marketplace => "marketplace",
            Self::Supporting => "supporting",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewResource {
    pub source_id: ManagedSourceId,
    pub upstream_key: String,
    pub kind: ResourceKind,
    pub resource_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewRevision {
    pub resource_id: ManagedResourceId,
    pub snapshot_id: SourceSnapshotId,
    pub parent_id: Option<ResourceRevisionId>,
    pub files: RevisionFiles,
    pub dependencies: BTreeMap<String, DependencyRef>,
    pub rationale: String,
}
