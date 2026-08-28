//! Authz-driven keep-set computation for a [`MarketplaceCandidate`].
//!
//! [`keep_sets`] resolves, per entry kind, which candidate ids the subject may
//! see, consulting `access_control_rules` through the security crate's bulk
//! resolver with the owning plugin and then the marketplace cascaded as
//! parents: the nearest level that declares any rule decides, so one plugin
//! rule covers every skill and artifact it ships. Extensions implementing
//! [`crate::MarketplaceFilter`] supply only the subject (roles, attributes,
//! dimensions) and pass the result to
//! [`MarketplaceCandidate::retain_entries`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::Hash;

use systemprompt_identifiers::{PluginId, SkillId, UserId};
use systemprompt_security::authz::{
    AccessControlRepository, BulkKeepQuery, ChainSources, EntityKind, MarketplaceSource,
    ParentChainIndex, SubjectAttributes, SubjectDimension, allowed_ids,
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
    let index = ParentChainIndex::load(repo, chain_sources(candidate))
        .await
        .map_err(|e| MarketplaceFilterError::Backend(e.to_string()))?;

    let allowed = |kind: EntityKind, ids: Vec<String>| {
        let chains = &index;
        async move {
            allowed_ids(
                repo,
                BulkKeepQuery {
                    user_id: subject.user_id,
                    roles: subject.roles,
                    kind,
                    ids: &ids,
                    chains,
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

// Why: skills stay marketplace members as well as plugin children, so a skill
// whose owners went unrecorded keeps the marketplace cascade instead of
// silently losing all inheritance.
fn chain_sources(candidate: &MarketplaceCandidate) -> ChainSources {
    let member_ids = |ids: Vec<String>| ids.into_iter().collect::<BTreeSet<String>>();
    let marketplace_members: BTreeMap<EntityKind, BTreeSet<String>> = [
        (EntityKind::Skill, ids_of(&candidate.skills, |s| &s.id)),
        (EntityKind::Agent, ids_of(&candidate.agents, |a| &a.id)),
        (EntityKind::Hook, ids_of(&candidate.hooks, |h| &h.id)),
        (
            EntityKind::McpServer,
            ids_of(&candidate.managed_mcp_servers, |m| &m.id),
        ),
    ]
    .into_iter()
    .map(|(kind, ids)| (kind, member_ids(ids)))
    .collect();

    ChainSources {
        marketplace: candidate
            .marketplace_id
            .clone()
            .map(|id| MarketplaceSource {
                id,
                fallback_default_included: candidate.access.as_ref().map(|a| a.default_included),
            }),
        plugins: candidate
            .plugins
            .iter()
            .map(|p| PluginId::new(p.id.as_str()))
            .collect(),
        skill_owners: candidate
            .skill_owners
            .iter()
            .map(|(skill, owners)| {
                (
                    SkillId::new(skill.as_str()),
                    owners.iter().map(|p| PluginId::new(p.as_str())).collect(),
                )
            })
            .collect(),
        marketplace_members,
    }
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
