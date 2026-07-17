//! Bootstrap-time projection of [`AccessControlConfig`] into the two-table
//! authz schema (`access_control_entities` + `access_control_rules`).
//!
//! This is the sanctioned YAML → DB ingestion path for authorization rules.
//! Direction is fixed (YAML → DB). Per-user overrides (`rule_type='user'`) are
//! runtime state and are *never* touched here, regardless of `delete_orphans`.
//!
//! Each rule's target is resolved before any write: a literal `entity_id` maps
//! to itself; an `entity_match` glob is expanded against the entities already
//! in the catalog for that kind (see [`super::config::RuleTarget`]). Every
//! resolved id is upserted into `access_control_entities` carrying the rule's
//! `default_included` flag — so the FK on `access_control_rules` is satisfied
//! and the resolver never sees the entity as `UnknownEntity`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod glob;
mod marketplace;
mod messaging;
mod upsert;

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

use super::config::{AccessControlConfig, RuleEntry, RuleTarget};
use super::error::{AuthzError, AuthzResult};
use super::types::{Access, EntityKind, RuleType};

use glob::glob_matches;
use upsert::{SOURCE_LABEL, Target, UpsertOutcome, upsert_entity_row, upsert_target};

#[derive(Debug, Clone, Copy, Default)]
pub struct IngestOptions {
    pub override_existing: bool,
    pub delete_orphans: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IngestReport {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone)]
pub struct AccessControlIngestionService {
    write_pool: Arc<PgPool>,
}

/// A rule with its glob expanded to concrete catalog ids, borrowing the role
/// list and justification from the source [`RuleEntry`].
struct ResolvedRule<'a> {
    entity_kind: EntityKind,
    ids: Vec<String>,
    access: &'static str,
    default_included: bool,
    roles: &'a [String],
    justification: Option<&'a str>,
}

impl AccessControlIngestionService {
    pub fn new(db: &DbPool) -> AuthzResult<Self> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|err| AuthzError::Validation(err.to_string()))?;
        Ok(Self { write_pool })
    }

    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { write_pool: pool }
    }

    /// `entity_match` globs are resolved against rows **already present** in
    /// `access_control_entities` for the rule's kind — entity bootstrap
    /// (publish pipeline, gateway reconciliation) must therefore run before
    /// ingestion, or a glob has nothing to expand over. Glob resolution reads
    /// on the same transaction as the subsequent writes, so a concurrent writer
    /// cannot insert a matching entity between the read and the commit.
    pub async fn ingest_config(
        &self,
        cfg: &AccessControlConfig,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        cfg.validate()?;

        let mut tx = self.write_pool.begin().await?;
        let resolved = Self::resolve_rules(&mut tx, &cfg.rules).await?;
        let mut report = IngestReport::default();

        if options.delete_orphans {
            // Why: `delete_orphans` clears stale role rules for the entities this
            // pass resolved — not the whole table. An unscoped sweep would race
            // any other writer (parallel test, concurrent bootstrap, another
            // tenant's loader) that owns role rules under a different entity.
            let mut entity_types: Vec<String> = Vec::new();
            let mut entity_ids: Vec<String> = Vec::new();
            for rule in &resolved {
                for id in &rule.ids {
                    entity_types.push(rule.entity_kind.as_str().to_owned());
                    entity_ids.push(id.clone());
                }
            }
            let res = sqlx::query!(
                r#"
                DELETE FROM access_control_rules
                WHERE rule_type = 'role'
                  AND (entity_type, entity_id) IN (
                      SELECT * FROM UNNEST($1::text[], $2::text[])
                  )
                "#,
                &entity_types,
                &entity_ids,
            )
            .execute(&mut *tx)
            .await?;
            report.deleted = res.rows_affected() as usize;
        }

        for rule in &resolved {
            for id in &rule.ids {
                upsert_entity_row(
                    &mut tx,
                    rule.entity_kind,
                    id,
                    rule.default_included,
                    SOURCE_LABEL,
                )
                .await?;
                for role in rule.roles {
                    let target = Target {
                        entity_kind: rule.entity_kind,
                        entity_id: id,
                        rule_type: RuleType::Role,
                        rule_value: role,
                        access: rule.access,
                        justification: rule.justification,
                    };
                    match upsert_target(&mut tx, &target, options.override_existing).await? {
                        UpsertOutcome::Inserted => report.inserted += 1,
                        UpsertOutcome::Updated => report.updated += 1,
                        UpsertOutcome::Skipped => report.skipped += 1,
                    }
                }
            }
        }

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_access_control_loaded",
            inserted = report.inserted,
            updated = report.updated,
            skipped = report.skipped,
            deleted = report.deleted,
            override_existing = options.override_existing,
            delete_orphans = options.delete_orphans,
            "access-control YAML ingested",
        );

        Ok(report)
    }

    async fn resolve_rules<'a>(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        rules: &'a [RuleEntry],
    ) -> AuthzResult<Vec<ResolvedRule<'a>>> {
        let mut catalog_cache: HashMap<EntityKind, Vec<String>> = HashMap::new();
        let mut out = Vec::with_capacity(rules.len());

        for rule in rules {
            let access = match rule.access {
                Access::Allow => "allow",
                Access::Deny => "deny",
            };
            let ids = match &rule.target {
                RuleTarget::Id(id) => vec![id.clone()],
                RuleTarget::Match(pattern) => {
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        catalog_cache.entry(rule.entity_type)
                    {
                        entry.insert(Self::list_entity_ids(tx, rule.entity_type).await?);
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

        Ok(out)
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
}
