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
//! and the resolver never sees the entity as `UnknownEntity`. For a kind the
//! caller enforces through [`RegisteredEntities`], a literal id outside the
//! registered set is rejected before any write instead of materialised.
//! Nothing is written when that check fails: resolution runs inside the
//! transaction but before every write, so the error rolls back an empty one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
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

pub use catalog::RegisteredEntities;
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

struct ResolvedRule<'a> {
    entity_kind: EntityKind,
    ids: Vec<String>,
    access: &'static str,
    default_included: bool,
    roles: &'a [String],
    justification: Option<&'a str>,
}

// Why: constructing this type is the only way to reach the write loop, so a
// future call site cannot persist rules that skipped the registry check.
struct ValidatedRules<'a>(Vec<ResolvedRule<'a>>);

impl<'a> ValidatedRules<'a> {
    fn rules(&self) -> &[ResolvedRule<'a>] {
        &self.0
    }
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

    pub async fn ingest_config_from_yaml_path(
        &self,
        yaml_path: &std::path::Path,
        options: IngestOptions,
        registered: &RegisteredEntities,
    ) -> AuthzResult<IngestReport> {
        let raw = std::fs::read_to_string(yaml_path).map_err(|err| {
            AuthzError::Validation(format!("failed to read {}: {err}", yaml_path.display()))
        })?;
        let cfg: AccessControlConfig = serde_yaml::from_str(&raw).map_err(|err| {
            AuthzError::Validation(format!(
                "failed to parse {} as AccessControlConfig: {err}",
                yaml_path.display()
            ))
        })?;
        self.ingest_config(&cfg, options, registered).await
    }

    pub async fn ingest_config(
        &self,
        cfg: &AccessControlConfig,
        options: IngestOptions,
        registered: &RegisteredEntities,
    ) -> AuthzResult<IngestReport> {
        cfg.validate()?;

        let mut tx = self.write_pool.begin().await?;
        let validated = Self::resolve_rules(&mut tx, &cfg.rules, registered).await?;
        let resolved = validated.rules();
        let mut report = IngestReport::default();

        if options.delete_orphans {
            let mut entity_types: Vec<String> = Vec::new();
            let mut entity_ids: Vec<String> = Vec::new();
            for rule in resolved {
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

        for rule in resolved {
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
                        rule_type: RuleType::ROLE,
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
                // Why: a literal id is the only target that can name something
                // that does not exist — a glob is expanded from the catalog, so
                // it cannot invent members. This is therefore the only branch
                // that needs checking, and the check has to happen here rather
                // than after the loop: `upsert_entity_row` below would mint the
                // row and make the id look real on the next run.
                RuleTarget::Id(id) => {
                    registered.require(rule.entity_type, id)?;
                    vec![id.clone()]
                },
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
}
