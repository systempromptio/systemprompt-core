//! Authz-driven keep-set computation for a [`MarketplaceCandidate`].
//!
//! [`keep_sets`] resolves, per entry kind, which candidate ids the subject may
//! see, consulting `access_control_rules` through the security crate's bulk
//! resolver with the owning marketplace cascaded as a parent: one marketplace
//! rule covers every member that declares no rules of its own, and a member
//! that declares any rule owns its decision outright. Extensions implementing
//! [`crate::MarketplaceFilter`] supply only the subject (roles, attributes,
//! dimensions) and pass the result to
//! [`MarketplaceCandidate::retain_entries`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::hash::Hash;

use systemprompt_identifiers::UserId;
use systemprompt_security::authz::{
    AccessControlRepository, BulkKeepQuery, EntityKind, MarketplaceParent, ResolveParent,
    SubjectAttributes, SubjectDimension, allowed_ids, load_marketplace_parent,
};

use crate::candidate::{EntryKeepSets, MarketplaceCandidate};
use crate::error::MarketplaceFilterError;

/// The subject whose visibility is being resolved. Attributes and dimensions
/// are gathered by the caller exactly as for `systemprompt_security::authz`'s
/// resolver.
#[derive(Debug, Clone, Copy)]
pub struct KeepSetsSubject<'a> {
    pub user_id: &'a UserId,
    pub roles: &'a [String],
    pub attributes: &'a SubjectAttributes,
    pub dimensions: &'a [SubjectDimension],
}

pub async fn keep_sets(
    repo: &AccessControlRepository,
    candidate: &MarketplaceCandidate,
    subject: KeepSetsSubject<'_>,
) -> Result<EntryKeepSets, MarketplaceFilterError> {
    let parent = match candidate.marketplace_id.as_ref() {
        Some(id) => Some(
            load_marketplace_parent(
                repo,
                id,
                candidate.access.as_ref().map(|a| a.default_included),
            )
            .await
            .map_err(|e| MarketplaceFilterError::Backend(e.to_string()))?,
        ),
        None => None,
    };
    let parents: Vec<ResolveParent<'_>> = parent
        .as_ref()
        .map(MarketplaceParent::as_resolve_parent)
        .into_iter()
        .collect();

    let allowed = |kind: EntityKind, ids: Vec<String>| {
        let parents = &parents;
        async move {
            allowed_ids(
                repo,
                BulkKeepQuery {
                    user_id: subject.user_id,
                    roles: subject.roles,
                    kind,
                    ids: &ids,
                    parents,
                    attributes: subject.attributes,
                    dimensions: subject.dimensions,
                },
            )
            .await
            .map_err(|e| MarketplaceFilterError::Backend(e.to_string()))
        }
    };

    let (plugins, skills, agents, hooks, mcp_servers) = tokio::try_join!(
        allowed(EntityKind::Plugin, ids_of(&candidate.plugins, |p| &p.id)),
        allowed(EntityKind::Skill, ids_of(&candidate.skills, |s| &s.id)),
        allowed(EntityKind::Agent, ids_of(&candidate.agents, |a| &a.id)),
        allowed(EntityKind::Hook, ids_of(&candidate.hooks, |h| &h.id)),
        allowed(
            EntityKind::McpServer,
            ids_of(&candidate.managed_mcp_servers, |m| &m.id),
        ),
    )?;

    Ok(EntryKeepSets {
        plugins: typed_keep(&candidate.plugins, &plugins, |p| &p.id),
        skills: typed_keep(&candidate.skills, &skills, |s| &s.id),
        agents: typed_keep(&candidate.agents, &agents, |a| &a.id),
        hooks: typed_keep(&candidate.hooks, &hooks, |h| &h.id),
        mcp_servers: typed_keep(&candidate.managed_mcp_servers, &mcp_servers, |m| &m.id),
    })
}

fn ids_of<T, Id: AsRef<str>>(items: &[T], id_of: impl Fn(&T) -> &Id) -> Vec<String> {
    items
        .iter()
        .map(|item| id_of(item).as_ref().to_owned())
        .collect()
}

fn typed_keep<T, Id>(
    items: &[T],
    allowed: &HashSet<String>,
    id_of: impl Fn(&T) -> &Id,
) -> HashSet<Id>
where
    Id: AsRef<str> + Clone + Eq + Hash,
{
    items
        .iter()
        .map(id_of)
        .filter(|id| allowed.contains(id.as_ref()))
        .cloned()
        .collect()
}
