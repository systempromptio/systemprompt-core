//! Owner-scoped persistence. Content writes are immutable and idempotent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod candidates;
pub use candidates::{RevisionComparison, TextCandidate};
mod listing;
pub use listing::{Page, ResourceSummary, RevisionSummary};
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
    pub(crate) pool: PgPool,
}

impl ManagedRepository {
    pub const PAGE_SIZE: i64 = 50;

    pub fn new(db: &systemprompt_database::DbPool) -> crate::managed::Result<Self> {
        Ok(Self {
            pool: db.write_pool_arc()?.as_ref().clone(),
        })
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, sqlx::Type,
)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewResource {
    pub source_id: ManagedSourceId,
    pub upstream_key: String,
    pub kind: ResourceKind,
    pub resource_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewRevision {
    pub resource_id: ManagedResourceId,
    pub snapshot_id: SourceSnapshotId,
    pub parent_id: Option<ResourceRevisionId>,
    pub files: RevisionFiles,
    pub dependencies: BTreeMap<String, DependencyRef>,
    pub rationale: String,
}
