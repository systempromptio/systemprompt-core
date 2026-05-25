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

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::RuleId;

use super::config::{AccessControlConfig, RuleEntry};
use super::error::{AuthzError, AuthzResult};
use super::types::{EntityKind, RuleType};

const SOURCE_LABEL: &str = "ingestion:access_control_config";

#[derive(Debug, Clone, Copy, Default)]
pub struct IngestOptions {
    pub override_existing: bool,
    pub delete_orphans: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IngestReport {
    pub departments_declared: usize,
    pub rules_inserted: usize,
    pub rules_updated: usize,
    pub rules_skipped: usize,
    pub rules_deleted: usize,
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
        let mut report = IngestReport {
            departments_declared: cfg.departments.len(),
            ..IngestReport::default()
        };

        if options.delete_orphans {
            // Why: `delete_orphans` clears stale role/department rules for the
            // entities the YAML declares — not for the entire table. An
            // unscoped sweep would race against any other writer (parallel
            // test, concurrent bootstrap, another tenant's loader) that owns
            // role/department rules under a different entity.
            let entity_types: Vec<String> = targets
                .iter()
                .map(|t| t.entity_kind.as_str().to_owned())
                .collect();
            let entity_ids: Vec<String> = targets.iter().map(|t| t.entity_id.to_owned()).collect();
            let res = sqlx::query!(
                r#"
                DELETE FROM access_control_rules
                WHERE rule_type IN ('role', 'department')
                  AND (entity_type, entity_id) IN (
                      SELECT * FROM UNNEST($1::text[], $2::text[])
                  )
                "#,
                &entity_types,
                &entity_ids,
            )
            .execute(&mut *tx)
            .await?;
            report.rules_deleted = res.rows_affected() as usize;
        }

        for target in &targets {
            upsert_entity_row(&mut tx, target).await?;
            let outcome = upsert_target(&mut tx, target, options.override_existing).await?;
            match outcome {
                UpsertOutcome::Inserted => report.rules_inserted += 1,
                UpsertOutcome::Updated => report.rules_updated += 1,
                UpsertOutcome::Skipped => report.rules_skipped += 1,
            }
        }

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_access_control_loaded",
            departments_declared = report.departments_declared,
            rules_inserted = report.rules_inserted,
            rules_updated = report.rules_updated,
            rules_skipped = report.rules_skipped,
            rules_deleted = report.rules_deleted,
            override_existing = options.override_existing,
            delete_orphans = options.delete_orphans,
            "access-control YAML ingested",
        );

        Ok(report)
    }
}

#[derive(Debug)]
struct Target<'a> {
    entity_kind: EntityKind,
    entity_id: &'a str,
    rule_type: RuleType,
    rule_value: &'a str,
    access: &'static str,
    justification: Option<&'a str>,
}

fn expand_targets(rules: &[RuleEntry]) -> Vec<Target<'_>> {
    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let access_str = match rule.access {
            super::types::Access::Allow => "allow",
            super::types::Access::Deny => "deny",
        };
        for role in &rule.roles {
            out.push(Target {
                entity_kind: rule.entity_type,
                entity_id: rule.entity_id.as_str(),
                rule_type: RuleType::Role,
                rule_value: role.as_str(),
                access: access_str,
                justification: rule.justification.as_deref(),
            });
        }
        for dept in &rule.departments {
            out.push(Target {
                entity_kind: rule.entity_type,
                entity_id: rule.entity_id.as_str(),
                rule_type: RuleType::Department,
                rule_value: dept.as_str(),
                access: access_str,
                justification: rule.justification.as_deref(),
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum UpsertOutcome {
    Inserted,
    Updated,
    Skipped,
}

async fn upsert_entity_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &Target<'_>,
) -> AuthzResult<()> {
    // Why: the rule FK requires the entity to exist; we never want to
    // clobber an existing default_included flag set by a higher-priority
    // loader (the publish-pipeline bootstrap pass), so this only inserts
    // missing rows.
    sqlx::query!(
        r#"
        INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
        VALUES ($1, $2, false, $3)
        ON CONFLICT (entity_type, entity_id) DO NOTHING
        "#,
        target.entity_kind.as_str(),
        target.entity_id,
        SOURCE_LABEL,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_target(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &Target<'_>,
    override_existing: bool,
) -> AuthzResult<UpsertOutcome> {
    let existing = sqlx::query!(
        r#"
        SELECT id, access, justification
        FROM access_control_rules
        WHERE entity_type = $1 AND entity_id = $2
          AND rule_type = $3 AND rule_value = $4
        "#,
        target.entity_kind.as_str(),
        target.entity_id,
        target.rule_type.to_string(),
        target.rule_value,
    )
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(row) = existing {
        if !override_existing {
            return Ok(UpsertOutcome::Skipped);
        }
        let unchanged =
            row.access == target.access && row.justification.as_deref() == target.justification;
        if unchanged {
            return Ok(UpsertOutcome::Skipped);
        }
        sqlx::query!(
            r#"
            UPDATE access_control_rules
            SET access = $2,
                justification = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
            row.id,
            target.access,
            target.justification,
        )
        .execute(&mut **tx)
        .await?;
        Ok(UpsertOutcome::Updated)
    } else {
        let id = RuleId::generate();
        sqlx::query!(
            r#"
            INSERT INTO access_control_rules
                (id, entity_type, entity_id, rule_type, rule_value, access, justification)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            id.as_str(),
            target.entity_kind.as_str(),
            target.entity_id,
            target.rule_type.to_string(),
            target.rule_value,
            target.access,
            target.justification,
        )
        .execute(&mut **tx)
        .await?;
        Ok(UpsertOutcome::Inserted)
    }
}
