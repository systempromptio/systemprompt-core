//! Dependency verification: the request a consumer submits and the manifest of
//! verified revisions the gateway answers with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DependencyVerificationId, ManagedSourceId, ResourceRevisionId};

use super::{ContentDigest, FeedbackContractError, validate_relative_path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencyVerificationInput {
    pub revision_id: ResourceRevisionId,
    pub source_id: ManagedSourceId,
    pub exact_commit: String,
    pub relative_root: String,
    pub dependencies: Vec<ResourceRevisionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependencyVerificationRequest {
    pub root_revision_id: ResourceRevisionId,
    pub revisions: Vec<DependencyVerificationInput>,
}

impl DependencyVerificationRequest {
    pub fn validate(&self) -> Result<(), FeedbackContractError> {
        if self.revisions.is_empty() || self.revisions.len() > 256 {
            return Err(FeedbackContractError::Bounds);
        }
        let mut graph = BTreeMap::new();
        for revision in &self.revisions {
            validate_relative_path(&revision.relative_root)?;
            if !matches!(revision.exact_commit.len(), 40 | 64)
                || !revision
                    .exact_commit
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || revision.dependencies.len() > 256
                || graph.insert(&revision.revision_id, revision).is_some()
            {
                return Err(FeedbackContractError::IncompleteManifest);
            }
        }
        let mut active = BTreeSet::new();
        let mut visited = BTreeSet::new();
        visit(&self.root_revision_id, &graph, &mut active, &mut visited)?;
        if visited.len() != graph.len() {
            return Err(FeedbackContractError::IncompleteManifest);
        }
        Ok(())
    }
}

fn visit<'a>(
    id: &'a ResourceRevisionId,
    graph: &BTreeMap<&'a ResourceRevisionId, &'a DependencyVerificationInput>,
    active: &mut BTreeSet<&'a ResourceRevisionId>,
    visited: &mut BTreeSet<&'a ResourceRevisionId>,
) -> Result<(), FeedbackContractError> {
    if active.contains(id) {
        return Err(FeedbackContractError::DependencyCycle);
    }
    if visited.contains(id) {
        return Ok(());
    }
    let node = graph
        .get(id)
        .ok_or(FeedbackContractError::IncompleteManifest)?;
    active.insert(id);
    let mut unique = BTreeSet::new();
    for dependency in &node.dependencies {
        if !unique.insert(dependency) {
            return Err(FeedbackContractError::IncompleteManifest);
        }
        visit(dependency, graph, active, visited)?;
    }
    active.remove(id);
    visited.insert(id);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VerifiedRevisionManifest {
    pub provenance: DependencyVerificationInput,
    pub content_digest: ContentDigest,
    pub file_count: u32,
    pub bytes_verified: bool,
    pub modes_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DependencyVerificationManifest {
    pub id: DependencyVerificationId,
    pub version: u16,
    pub root_revision_id: ResourceRevisionId,
    pub bundle_digest: ContentDigest,
    pub revisions: Vec<VerifiedRevisionManifest>,
    pub verified_at: DateTime<Utc>,
}

impl DependencyVerificationManifest {
    pub fn validate_complete(&self) -> Result<(), FeedbackContractError> {
        if self.version != 1
            || self.revisions.iter().any(|revision| {
                !revision.bytes_verified || !revision.modes_verified || revision.file_count == 0
            })
        {
            return Err(FeedbackContractError::IncompleteManifest);
        }
        DependencyVerificationRequest {
            root_revision_id: self.root_revision_id.clone(),
            revisions: self
                .revisions
                .iter()
                .map(|revision| revision.provenance.clone())
                .collect(),
        }
        .validate()
    }
}
