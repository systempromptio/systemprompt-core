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
//! An entry owned by several enabled marketplaces gets one chain per owner, so
//! any admitting marketplace admits it; a rule on the entry or its plugin is
//! evaluated first and closes the cascade, so a deny there still wins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::Hash;

use systemprompt_identifiers::{MarketplaceId, PluginId, SkillId, UserId};
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
    let index = ParentChainIndex::load(repo, std::sync::Arc::new(chain_sources(candidate)))
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

// Why: skills and artifacts inherit through the plugins that ship them, so an
// entry no plugin claims falls back to every enabled marketplace rather than
// silently losing all inheritance.
fn chain_sources(candidate: &MarketplaceCandidate) -> ChainSources {
    let membership = &candidate.membership;
    let all = membership.all_ids();

    let marketplaces = membership
        .access
        .iter()
        .map(|(id, access)| {
            (
                id.clone(),
                MarketplaceSource {
                    id: id.clone(),
                    fallback_default_included: Some(access.default_included),
                },
            )
        })
        .collect();

    let plugins: BTreeMap<PluginId, BTreeSet<MarketplaceId>> = candidate
        .plugins
        .iter()
        .map(|p| {
            let id = PluginId::new(p.id.as_str());
            let owners = membership
                .plugins
                .get(&id)
                .cloned()
                .unwrap_or_else(|| all.clone());
            (id, owners)
        })
        .collect();

    let band = |ids: Vec<String>| -> BTreeMap<String, BTreeSet<MarketplaceId>> {
        ids.into_iter().map(|id| (id, all.clone())).collect()
    };
    let named = |owners: &BTreeMap<String, BTreeSet<MarketplaceId>>,
                 ids: Vec<String>|
     -> BTreeMap<String, BTreeSet<MarketplaceId>> {
        ids.into_iter()
            .map(|id| {
                let set = owners.get(&id).cloned().unwrap_or_else(|| all.clone());
                (id, set)
            })
            .collect()
    };

    let agent_owners: BTreeMap<String, BTreeSet<MarketplaceId>> = membership
        .agents
        .iter()
        .map(|(id, set)| (id.as_str().to_owned(), set.clone()))
        .collect();
    let mcp_owners: BTreeMap<String, BTreeSet<MarketplaceId>> = membership
        .mcp_servers
        .iter()
        .map(|(id, set)| (id.as_str().to_owned(), set.clone()))
        .collect();

    let marketplace_members = BTreeMap::from([
        (
            EntityKind::Skill,
            band(ids_of(&candidate.skills, |s| &s.id)),
        ),
        (
            EntityKind::Agent,
            named(&agent_owners, ids_of(&candidate.agents, |a| &a.id)),
        ),
        (EntityKind::Hook, band(ids_of(&candidate.hooks, |h| &h.id))),
        (
            EntityKind::McpServer,
            named(
                &mcp_owners,
                ids_of(&candidate.managed_mcp_servers, |m| &m.id),
            ),
        ),
    ]);

    ChainSources {
        marketplaces,
        plugins,
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
