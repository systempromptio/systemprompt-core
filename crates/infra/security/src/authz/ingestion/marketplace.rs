//! Projection of each marketplace's declarative `access` block into the
//! marketplace-scoped authz rows, reusing the role-rule upsert path that
//! [`super::AccessControlIngestionService::ingest_config`] uses for YAML rules.
//!
//! `access.roles` projects into the `role` band; each `access.rules` entry
//! projects one row per value into the extension subject-dimension band it
//! names, so a group or project grant is enforced by the same resolver as a
//! role.
//!
//! Orphan deletion owns only the `(entity_id, rule_type)` pairs the config
//! still declares. Within a declared band the rows are pruned to exactly what
//! the config names; a band the config no longer mentions is left in place,
//! because a delete scoped by entity alone would take rules another writer owns
//! with it. Removing a whole band from the YAML therefore does not revoke it —
//! that is a deliberate deletion, made with the CLI or by hand.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::MarketplaceConfig;

use super::super::error::{AuthzError, AuthzResult};
use super::super::types::{EntityKind, RuleType};
use super::upsert::{Target, UpsertOutcome, upsert_marketplace_entity_row, upsert_target};
use super::{AccessControlIngestionService, IngestOptions, IngestReport};

struct DeclaredBands {
    entity_ids: Vec<String>,
    rule_types: Vec<String>,
}

fn declared_bands(marketplaces: &HashMap<MarketplaceId, MarketplaceConfig>) -> DeclaredBands {
    let mut entity_ids = Vec::new();
    let mut rule_types = Vec::new();
    for (id, cfg) in marketplaces {
        for rule_type in cfg.access.rule_types() {
            entity_ids.push(id.as_str().to_owned());
            rule_types.push(rule_type.to_owned());
        }
    }
    DeclaredBands {
        entity_ids,
        rule_types,
    }
}

const fn tally(report: &mut IngestReport, outcome: UpsertOutcome) {
    match outcome {
        UpsertOutcome::Inserted => report.inserted += 1,
        UpsertOutcome::Updated => report.updated += 1,
        UpsertOutcome::Skipped => report.skipped += 1,
    }
}

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

// Why: every rule type is checked before the transaction opens, so a malformed
// slug cannot leave half a marketplace's grants written.
fn validate_rule_types(
    marketplaces: &HashMap<MarketplaceId, MarketplaceConfig>,
) -> AuthzResult<()> {
    for (id, cfg) in marketplaces {
        for rule in &cfg.access.rules {
            RuleType::extension(rule.rule_type.clone()).map_err(|source| {
                AuthzError::Validation(format!(
                    "marketplace '{}': access.rules rule_type '{}' is not a valid subject \
                     dimension: {source}",
                    id.as_str(),
                    rule.rule_type
                ))
            })?;
        }
    }
    Ok(())
}

async fn delete_declared_bands(tx: &mut Tx<'_>, bands: &DeclaredBands) -> AuthzResult<usize> {
    let res = sqlx::query!(
        r#"
        DELETE FROM access_control_rules
        WHERE entity_type = 'marketplace'
          AND (entity_id, rule_type) IN (
              SELECT * FROM UNNEST($1::text[], $2::text[])
          )
        "#,
        &bands.entity_ids,
        &bands.rule_types,
    )
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected() as usize)
}

async fn upsert_marketplace(
    tx: &mut Tx<'_>,
    entity_id: &str,
    cfg: &MarketplaceConfig,
    options: IngestOptions,
    report: &mut IngestReport,
) -> AuthzResult<()> {
    upsert_marketplace_entity_row(tx, entity_id, cfg.access.default_included).await?;

    for role in &cfg.access.roles {
        let target = Target {
            entity_kind: EntityKind::Marketplace,
            entity_id,
            rule_type: RuleType::ROLE,
            rule_value: role.as_str(),
            access: "allow",
            justification: cfg.access.justification.as_deref(),
        };
        let outcome = upsert_target(tx, &target, options.override_existing).await?;
        tally(report, outcome);
    }

    for rule in &cfg.access.rules {
        let rule_type = RuleType::extension(rule.rule_type.clone())
            .map_err(|source| AuthzError::Validation(source.to_string()))?;
        let justification = rule
            .justification
            .as_deref()
            .or(cfg.access.justification.as_deref());
        for value in &rule.values {
            let target = Target {
                entity_kind: EntityKind::Marketplace,
                entity_id,
                rule_type: rule_type.clone(),
                rule_value: value.as_str(),
                access: rule.access.as_str(),
                justification,
            };
            let outcome = upsert_target(tx, &target, options.override_existing).await?;
            tally(report, outcome);
        }
    }

    Ok(())
}

impl AccessControlIngestionService {
    pub async fn ingest_marketplace_access(
        &self,
        marketplaces: &HashMap<MarketplaceId, MarketplaceConfig>,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        validate_rule_types(marketplaces)?;

        let mut tx = self.write_pool.begin().await?;
        let mut report = IngestReport::default();

        let bands = declared_bands(marketplaces);
        if options.delete_orphans && !bands.entity_ids.is_empty() {
            report.deleted = delete_declared_bands(&mut tx, &bands).await?;
        }

        for (id, cfg) in marketplaces {
            if !cfg.access.declares_rules() {
                continue;
            }
            upsert_marketplace(&mut tx, id.as_str(), cfg, options, &mut report).await?;
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
