//! Bootstrap-time projection of [`AccessControlConfig`] into the two-table
//! authz schema (`access_control_entities` + `access_control_rules`).
//!
//! This is the only sanctioned YAML → DB ingestion path for authorization
//! rules. Direction is fixed (YAML → DB). There is no opposite. Per-user
//! overrides (`rule_type='user'`) are runtime state and are *never* touched
//! here, regardless of `delete_orphans`.
//!
//! Every rule's `entity_id` is also upserted into `access_control_entities`
//! with `default_included = false` so the FK on `access_control_rules` is
//! satisfied and the resolver does not treat the entity as `UnknownEntity`.

mod upsert;

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::MarketplaceConfig;

use super::config::AccessControlConfig;
use super::error::{AuthzError, AuthzResult};
use super::types::{EntityKind, RuleType};

use upsert::{
    Target, UpsertOutcome, expand_targets, upsert_entity_row, upsert_marketplace_entity_row,
    upsert_target,
};

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

    pub async fn ingest_config(
        &self,
        cfg: &AccessControlConfig,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        cfg.validate()?;

        let targets = expand_targets(&cfg.rules);

        let mut tx = self.write_pool.begin().await?;
        let mut report = IngestReport::default();

        if options.delete_orphans {
            // Why: `delete_orphans` clears stale role rules for the entities
            // the YAML declares — not for the entire table. An unscoped sweep
            // would race against any other writer (parallel test, concurrent
            // bootstrap, another tenant's loader) that owns role rules under
            // a different entity.
            let entity_types: Vec<String> = targets
                .iter()
                .map(|t| t.entity_kind.as_str().to_owned())
                .collect();
            let entity_ids: Vec<String> = targets.iter().map(|t| t.entity_id.to_owned()).collect();
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

        for target in &targets {
            upsert_entity_row(&mut tx, target).await?;
            let outcome = upsert_target(&mut tx, target, options.override_existing).await?;
            match outcome {
                UpsertOutcome::Inserted => report.inserted += 1,
                UpsertOutcome::Updated => report.updated += 1,
                UpsertOutcome::Skipped => report.skipped += 1,
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

    /// Projects each marketplace's declarative `access` block into
    /// marketplace-scoped `access_control_entities` / `access_control_rules`
    /// rows, reusing the same role-rule upsert path as [`Self::ingest_config`].
    ///
    /// Only `access.roles` and `access.default_included` cross the boundary —
    /// the opaque `access.attributes` bag is never ingested; it is forwarded
    /// verbatim to extension ABAC hooks elsewhere. Marketplaces with no roles
    /// are skipped entirely (no entity row is written for them here).
    pub async fn ingest_marketplace_access(
        &self,
        marketplaces: &HashMap<MarketplaceId, MarketplaceConfig>,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        let mut tx = self.write_pool.begin().await?;
        let mut report = IngestReport::default();

        let mut ingested_ids: Vec<String> = Vec::new();
        for (id, cfg) in marketplaces {
            if cfg.access.roles.is_empty() {
                continue;
            }
            ingested_ids.push(id.as_str().to_owned());
        }

        if options.delete_orphans && !ingested_ids.is_empty() {
            // Why: scope the sweep to the marketplaces this pass actually owns,
            // mirroring the role-rule path in `ingest_config`; an unscoped
            // delete would race other writers holding marketplace role rules.
            let res = sqlx::query!(
                r#"
                DELETE FROM access_control_rules
                WHERE rule_type = 'role'
                  AND entity_type = 'marketplace'
                  AND entity_id = ANY($1::text[])
                "#,
                &ingested_ids,
            )
            .execute(&mut *tx)
            .await?;
            report.deleted = res.rows_affected() as usize;
        }

        for (id, cfg) in marketplaces {
            if cfg.access.roles.is_empty() {
                continue;
            }
            let entity_id = id.as_str();
            upsert_marketplace_entity_row(&mut tx, entity_id, cfg.access.default_included).await?;
            for role in &cfg.access.roles {
                let target = Target {
                    entity_kind: EntityKind::Marketplace,
                    entity_id,
                    rule_type: RuleType::Role,
                    rule_value: role.as_str(),
                    access: "allow",
                    justification: cfg.access.justification.as_deref(),
                };
                let outcome = upsert_target(&mut tx, &target, options.override_existing).await?;
                match outcome {
                    UpsertOutcome::Inserted => report.inserted += 1,
                    UpsertOutcome::Updated => report.updated += 1,
                    UpsertOutcome::Skipped => report.skipped += 1,
                }
            }
        }

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_marketplace_access_loaded",
            inserted = report.inserted,
            updated = report.updated,
            skipped = report.skipped,
            deleted = report.deleted,
            override_existing = options.override_existing,
            delete_orphans = options.delete_orphans,
            "marketplace access blocks ingested",
        );

        Ok(report)
    }
}

