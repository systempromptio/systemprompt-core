//! The `entity → plugin → marketplace` parent chain, loaded through
//! [`ChainIndexCache`] and shared by every enforcement site.
//!
//! [`ChainSources`] says who parents whom; [`ParentChainIndex::load`] fetches
//! the rules and `default_included` sentinels for the marketplace and every
//! plugin named in four bulk queries, independent of catalogue size.
//! [`ParentChainIndex::resolve`] then runs the pure [`resolve`] resolver once
//! per owner chain: a skill selected by several plugins is admitted when any
//! owner admits the subject, mirroring how an artifact survives while any
//! plugin shipping it survives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cache;
mod sources;

use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_identifiers::{PluginId, UserId};

use super::error::AuthzResult;
use super::repository::AccessControlRepository;
use super::resolver::{ResolveInput, ResolveParent, resolve};
use super::subject::{SubjectAttributes, SubjectDimension};
use super::types::{AccessRule, Decision, EntityKind, EntityRef};

pub use cache::ChainIndexCache;
pub use sources::{ChainSources, MarketplaceSource};

#[derive(Debug, Clone)]
pub struct LoadedParent {
    pub entity: EntityRef,
    pub rules: Vec<AccessRule>,
    pub default_included: Option<bool>,
}

impl LoadedParent {
    #[must_use]
    pub fn as_resolve_parent(&self) -> ResolveParent<'_> {
        ResolveParent {
            entity: &self.entity,
            rules: &self.rules,
            default_included: self.default_included,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolveBase<'a> {
    pub rules: &'a [AccessRule],
    pub user_id: &'a UserId,
    pub user_roles: &'a [String],
    pub default_included: Option<bool>,
    pub attributes: &'a SubjectAttributes,
    pub dimensions: &'a [SubjectDimension],
}

#[derive(Debug, Clone, Default)]
pub struct ParentChainIndex {
    marketplace: Option<LoadedParent>,
    plugins: BTreeMap<PluginId, LoadedParent>,
    // Why: shared rather than owned. The sources are fixed for the process
    // lifetime while the index is rebuilt whenever the cache sees the rule or
    // entity tables change, so an owned copy would deep-clone every plugin
    // id, skill id and member set on each rebuild for no benefit.
    sources: Arc<ChainSources>,
}

impl ParentChainIndex {
    #[must_use]
    pub const fn from_parts(
        marketplace: Option<LoadedParent>,
        plugins: BTreeMap<PluginId, LoadedParent>,
        sources: Arc<ChainSources>,
    ) -> Self {
        Self {
            marketplace,
            plugins,
            sources,
        }
    }

    pub async fn load(
        repo: &AccessControlRepository,
        sources: Arc<ChainSources>,
    ) -> AuthzResult<Self> {
        let marketplace = match sources.marketplace.as_ref() {
            Some(source) => Some(load_marketplace(repo, source).await?),
            None => None,
        };

        let plugin_ids = sources.plugin_ids_to_load();
        let mut rules = repo
            .list_rules_bulk(EntityKind::Plugin, &plugin_ids)
            .await?;
        let entities = repo
            .list_entities_bulk(EntityKind::Plugin, &plugin_ids)
            .await?;
        let plugins = plugin_ids
            .into_iter()
            .map(|id| {
                let parent = LoadedParent {
                    entity: EntityRef::from_kind_and_id(EntityKind::Plugin, &id),
                    rules: rules.remove(&id).unwrap_or_default(),
                    default_included: entities.get(&id).map(|row| row.default_included),
                };
                (PluginId::new(id), parent)
            })
            .collect();

        Ok(Self {
            marketplace,
            plugins,
            sources,
        })
    }

    #[must_use]
    pub fn chains_for(&self, kind: EntityKind, id: &str) -> Vec<Vec<ResolveParent<'_>>> {
        let marketplace = self
            .marketplace
            .as_ref()
            .map(LoadedParent::as_resolve_parent);
        let marketplace_chain = || {
            marketplace
                .map(|parent| vec![vec![parent]])
                .unwrap_or_default()
        };

        match kind {
            EntityKind::Skill => {
                let owned: Vec<Vec<ResolveParent<'_>>> = self
                    .sources
                    .skill_owners
                    .get(id)
                    .into_iter()
                    .flatten()
                    .filter_map(|owner| self.plugins.get(owner).map(|plugin| (owner, plugin)))
                    .map(|(owner, plugin)| {
                        let mut chain = vec![plugin.as_resolve_parent()];
                        if self.sources.plugins.contains(owner) {
                            chain.extend(marketplace);
                        }
                        chain
                    })
                    .collect();
                if !owned.is_empty() {
                    return owned;
                }
                if self.sources.is_marketplace_member(kind, id) {
                    return marketplace_chain();
                }
                Vec::new()
            },
            EntityKind::Plugin if self.sources.plugins.contains(id) => marketplace_chain(),
            EntityKind::Plugin => Vec::new(),
            _ if self.sources.is_marketplace_member(kind, id) => marketplace_chain(),
            _ => Vec::new(),
        }
    }

    #[must_use]
    pub fn resolve(&self, kind: EntityKind, id: &str, base: ResolveBase<'_>) -> Decision {
        let entity = EntityRef::from_kind_and_id(kind, id);
        let resolve_with = |parents: &[ResolveParent<'_>]| {
            resolve(ResolveInput {
                entity: &entity,
                rules: base.rules,
                user_id: base.user_id,
                user_roles: base.user_roles,
                default_included: base.default_included,
                parents,
                attributes: base.attributes,
                dimensions: base.dimensions,
            })
        };

        let mut first_deny = None;
        for chain in self.chains_for(kind, id) {
            let decision = resolve_with(&chain);
            if matches!(decision, Decision::Allow { .. }) {
                return decision;
            }
            first_deny.get_or_insert(decision);
        }
        first_deny.unwrap_or_else(|| resolve_with(&[]))
    }
}

async fn load_marketplace(
    repo: &AccessControlRepository,
    source: &MarketplaceSource,
) -> AuthzResult<LoadedParent> {
    let id = source.id.as_str();
    let rules = repo
        .list_rules_for_entity(EntityKind::Marketplace, id)
        .await?;
    let default_included = repo
        .get_entity(EntityKind::Marketplace, id)
        .await?
        .map(|row| row.default_included)
        .or(source.fallback_default_included);
    Ok(LoadedParent {
        entity: EntityRef::Marketplace(source.id.clone()),
        rules,
        default_included,
    })
}
