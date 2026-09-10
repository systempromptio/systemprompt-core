//! Rule-target resolution and the scoped role-rule prune.
//!
//! Resolution runs inside the ingest transaction but ahead of every write, so
//! an unregistered literal id rolls back an empty transaction rather than a
//! half-applied one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use super::super::config::{RuleEntry, RuleTarget};
use super::super::error::AuthzResult;
use super::super::types::{Access, EntityKind};
use super::glob::glob_matches;
use super::{IngestOptions, RegisteredEntities};

pub(super) struct ResolvedRule<'a> {
    pub(super) entity_kind: EntityKind,
    pub(super) ids: Vec<String>,
    pub(super) access: &'static str,
    pub(super) default_included: bool,
    pub(super) roles: &'a [String],
    pub(super) justification: Option<&'a str>,
}

pub(super) struct ValidatedRules<'a>(Vec<ResolvedRule<'a>>);

impl<'a> ValidatedRules<'a> {
    pub(super) fn rules(&self) -> &[ResolvedRule<'a>] {
        &self.0
    }
}

pub(super) async fn prune_role_rules(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resolved: &[ResolvedRule<'_>],
    options: &IngestOptions,
) -> AuthzResult<usize> {
    let mut entity_types: Vec<String> = Vec::new();
    let mut entity_ids: Vec<String> = Vec::new();
    for rule in resolved {
        for id in &rule.ids {
            if !options.scope.owns(rule.entity_kind, id) {
                continue;
            }
            entity_types.push(rule.entity_kind.as_str().to_owned());
            entity_ids.push(id.clone());
        }
    }
    let res = sqlx::query!(
        r#"
        DELETE FROM access_control_rules
        WHERE rule_type = 'role'
          AND source = $3
          AND (entity_type, entity_id) IN (
              SELECT * FROM UNNEST($1::text[], $2::text[])
          )
        "#,
        &entity_types,
        &entity_ids,
        options.source,
    )
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected() as usize)
}

pub(super) async fn resolve_rules<'a>(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rules: &'a [RuleEntry],
    registered: &RegisteredEntities,
) -> AuthzResult<ValidatedRules<'a>> {
    let mut catalog_cache: HashMap<EntityKind, Vec<String>> = HashMap::new();
    let mut out = Vec::with_capacity(rules.len());

    for rule in rules {
        let access = match rule.access {
            Access::Allow => "allow",
            Access::Deny => "deny",
        };
        let ids = match &rule.target {
            RuleTarget::Id(id) => {
                registered.require(rule.entity_type, id)?;
                vec![id.clone()]
            },
            RuleTarget::Match(pattern) => {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    catalog_cache.entry(rule.entity_type)
                {
                    entry.insert(list_entity_ids(tx, rule.entity_type).await?);
                }
                catalog_cache[&rule.entity_type]
                    .iter()
                    .filter(|id| glob_matches(pattern, id))
                    .cloned()
                    .collect()
            },
        };
        out.push(ResolvedRule {
            entity_kind: rule.entity_type,
            ids,
            access,
            default_included: rule.default_included,
            roles: &rule.roles,
            justification: rule.justification.as_deref(),
        });
    }

    Ok(ValidatedRules(out))
}

async fn list_entity_ids(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: EntityKind,
) -> AuthzResult<Vec<String>> {
    let rows = sqlx::query!(
        r#"
        SELECT entity_id
        FROM access_control_entities
        WHERE entity_type = $1
        "#,
        kind.as_str(),
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows.into_iter().map(|row| row.entity_id).collect())
}
