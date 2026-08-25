//! Bulk allow-resolution over one entity kind.
//!
//! [`allowed_ids`] answers "which of these candidate ids may this subject
//! see?" in two queries (rules + entity sentinels) instead of a per-id
//! lookup, then runs the pure [`resolve`] resolver per id. It is the shared
//! engine behind per-user catalogue filtering (marketplace manifests, admin
//! screens); callers supply the subject's attributes and dimensions exactly
//! as they would to [`resolve`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::UserId;

use super::error::AuthzResult;
use super::repository::AccessControlRepository;
use super::resolver::{ResolveInput, ResolveParent, resolve};
use super::subject::{SubjectAttributes, SubjectDimension};
use super::types::{Decision, EntityKind, EntityRef};

/// Inputs to [`allowed_ids`]: one subject, one entity kind, many candidates.
#[derive(Debug, Clone, Copy)]
pub struct BulkKeepQuery<'a> {
    pub user_id: &'a UserId,
    pub roles: &'a [String],
    pub kind: EntityKind,
    pub ids: &'a [String],
    pub parents: &'a [ResolveParent<'a>],
    pub attributes: &'a SubjectAttributes,
    pub dimensions: &'a [SubjectDimension],
}

pub async fn allowed_ids(
    repo: &AccessControlRepository,
    query: BulkKeepQuery<'_>,
) -> AuthzResult<HashSet<String>> {
    if query.ids.is_empty() {
        return Ok(HashSet::new());
    }
    let rules = repo.list_rules_bulk(query.kind, query.ids).await?;
    let entities = repo.list_entities_bulk(query.kind, query.ids).await?;
    let mut keep = HashSet::with_capacity(query.ids.len());
    for id in query.ids {
        let entity_rules = rules.get(id).map_or(&[][..], Vec::as_slice);
        let default_included = entities.get(id).map(|e| e.default_included);
        let entity = EntityRef::from_kind_and_id(query.kind, id);
        let decision = resolve(ResolveInput {
            entity: &entity,
            rules: entity_rules,
            user_id: query.user_id,
            user_roles: query.roles,
            default_included,
            parents: query.parents,
            attributes: query.attributes,
            dimensions: query.dimensions,
        });
        if matches!(decision, Decision::Allow { .. }) {
            keep.insert(id.clone());
        }
    }
    Ok(keep)
}
