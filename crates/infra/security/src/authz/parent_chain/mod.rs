//! The `entity → plugin → marketplace` parent chain, loaded through
//! [`ChainIndexCache`] and shared by every enforcement site.
//!
//! [`ChainSources`] says who parents whom; [`ParentChainIndex::load`] fetches
//! the rules and `default_included` sentinels for every enabled marketplace
//! and every named plugin in four bulk queries, independent of catalogue size.
//! [`ParentChainIndex::resolve`] then runs the pure [`resolve`] resolver once
//! per owner chain: an entity that belongs to several marketplaces, or a skill
//! selected by several plugins, is admitted when any one of them admits the
//! subject, mirroring how an artifact survives while any plugin shipping it
//! survives.
//!
//! The entity's own ruleset is evaluated ahead of its parents, and a plugin
//! rule closes the cascade before any marketplace is consulted, so a deny at
//! the entity or plugin level still wins over every admitting marketplace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cache;
mod chains;
mod sources;

use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_identifiers::{MarketplaceId, PluginId, UserId};

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
    pub(super) marketplaces: BTreeMap<MarketplaceId, LoadedParent>,
    pub(super) plugins: BTreeMap<PluginId, LoadedParent>,
    // Why: shared rather than owned. The sources are fixed for the process
    // lifetime while the index is rebuilt whenever the cache sees the rule or
    // entity tables change, so an owned copy would deep-clone every plugin
    // id, skill id and member set on each rebuild for no benefit.
    pub(super) sources: Arc<ChainSources>,
}

impl ParentChainIndex {
    #[must_use]
    pub const fn from_parts(
        marketplaces: BTreeMap<MarketplaceId, LoadedParent>,
        plugins: BTreeMap<PluginId, LoadedParent>,
        sources: Arc<ChainSources>,
    ) -> Self {
        Self {
            marketplaces,
            plugins,
            sources,
        }
    }

    pub async fn load(
        repo: &AccessControlRepository,
        sources: Arc<ChainSources>,
    ) -> AuthzResult<Self> {
        let marketplaces = load_marketplaces(repo, &sources).await?;

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
            marketplaces,
            plugins,
            sources,
        })
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
            if decision.permits() {
                return decision;
            }
            first_deny.get_or_insert(decision);
        }
        first_deny.unwrap_or_else(|| resolve_with(&[]))
    }
}

async fn load_marketplaces(
    repo: &AccessControlRepository,
    sources: &ChainSources,
) -> AuthzResult<BTreeMap<MarketplaceId, LoadedParent>> {
    let ids = sources.marketplace_ids_to_load();
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let raw: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
    let mut rules = repo.list_rules_bulk(EntityKind::Marketplace, &raw).await?;
    let entities = repo
        .list_entities_bulk(EntityKind::Marketplace, &raw)
        .await?;
    Ok(ids
        .into_iter()
        .map(|id| {
            let fallback = sources
                .marketplaces
                .get(&id)
                .and_then(|source| source.fallback_default_included);
            let parent = LoadedParent {
                entity: EntityRef::Marketplace(id.clone()),
                rules: rules.remove(id.as_str()).unwrap_or_default(),
                default_included: entities
                    .get(id.as_str())
                    .map(|row| row.default_included)
                    .or(fallback),
            };
            (id, parent)
        })
        .collect())
}
