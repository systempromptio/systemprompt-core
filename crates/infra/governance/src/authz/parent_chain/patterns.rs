//! Decision-time pattern and entitlement levels (credential.md §3.1 rule 7).
//!
//! The access plane stores `gateway_route/*`-style globs, `model/<glob>` and
//! `provider/<name>` as written; nothing is expanded at apply. They are loaded
//! with the chain index and matched here against the request, so a route or
//! model added after the last apply is covered on its first request. For a
//! gateway route they yield, nearest first: the `model` entities matching the
//! requested model (an exact id before any glob, a longer glob before a
//! shorter one), the `provider` entity naming the provider serving it, then
//! the route globs matching the route id; any other entity gets its own
//! kind's globs. A request no declared route matched is decided on the
//! `model` entity itself: its own rules, the `model` globs matching it, then
//! the `provider` entity. Entitlements come before route globs so a
//! `model/<glob>` narrows a blanket `gateway_route/*` grant rather than being
//! shadowed by it. All of them sit between the entity's own rules and its
//! marketplace chain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cmp::Reverse;

use super::{LoadedParent, ParentChainIndex};
use crate::authz::error::{AuthzError, AuthzResult};
use crate::authz::ingestion::glob::glob_matches;
use crate::authz::repository::AccessControlRepository;
use crate::authz::resolver::ResolveParent;
use crate::authz::types::{EntityKind, EntityRef};

/// What a decision knows beyond the entity.
///
/// For a gateway route, the requested model and the provider serving it; for a
/// document folder, the folders above it nearest-first. A deny on any of them
/// governs the whole subtree and no nearer ruleset overrides it; their
/// allows cascade nearest-first, and their `default_included` sentinels are
/// never consulted.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecisionFacts<'a> {
    pub model: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub folder_ancestors: &'a [ResolveParent<'a>],
}

pub(super) async fn load_patterns(
    repo: &AccessControlRepository,
) -> AuthzResult<Vec<LoadedParent>> {
    let entities = repo.list_pattern_entities().await?;
    let mut out = Vec::with_capacity(entities.len());
    for kind in EntityKind::ALL {
        let ids: Vec<String> = entities
            .iter()
            .filter(|row| row.kind == *kind)
            .map(|row| row.id.clone())
            .collect();
        if ids.is_empty() {
            continue;
        }
        let mut rules = repo.list_rules_bulk(*kind, &ids).await?;
        for row in entities.iter().filter(|row| row.kind == *kind) {
            out.push(LoadedParent {
                entity: EntityRef::from_kind_and_id(*kind, &row.id)
                    .map_err(AuthzError::InvalidEntityId)?,
                rules: rules.remove(&row.id).unwrap_or_default(),
                default_included: Some(row.default_included),
            });
        }
    }
    Ok(out)
}

impl ParentChainIndex {
    #[must_use]
    pub fn pattern_parents(
        &self,
        kind: EntityKind,
        id: &str,
        facts: DecisionFacts<'_>,
    ) -> Vec<ResolveParent<'_>> {
        let mut parents = Vec::new();
        if kind == EntityKind::GatewayRoute
            && let Some(model) = facts.model
        {
            parents
                .extend(self.matching(EntityKind::Model, |pattern| glob_matches(pattern, model)));
        }
        let own_globs = self.matching(kind, |pattern| {
            pattern != id && pattern.contains('*') && glob_matches(pattern, id)
        });
        let provider = facts
            .provider
            .filter(|_| matches!(kind, EntityKind::GatewayRoute | EntityKind::Model))
            .map(|provider| self.matching(EntityKind::Provider, |name| name == provider))
            .unwrap_or_default();
        if kind == EntityKind::Model {
            parents.extend(own_globs);
            parents.extend(provider);
        } else {
            parents.extend(provider);
            parents.extend(own_globs);
        }
        parents
    }

    fn matching(&self, kind: EntityKind, matches: impl Fn(&str) -> bool) -> Vec<ResolveParent<'_>> {
        let mut found: Vec<&LoadedParent> = self
            .patterns
            .iter()
            .filter(|p| p.entity.kind() == kind && matches(p.entity.id_str()))
            .collect();
        found.sort_by_key(|p| {
            let id = p.entity.id_str();
            (id.contains('*'), Reverse(id.len()))
        });
        found
            .into_iter()
            .map(LoadedParent::as_resolve_parent)
            .collect()
    }
}
