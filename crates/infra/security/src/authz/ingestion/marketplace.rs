//! Projection of each marketplace's declarative `access` block into the
//! marketplace-scoped authz rows, reusing the role-rule upsert path that
//! [`super::AccessControlIngestionService::ingest_config`] uses for YAML rules.
//!
//! `access.roles` projects into the `role` band; each `access.rules` entry
//! projects one row per value into the extension subject-dimension band it
//! names, so a group or project grant is enforced by the same resolver as a
//! role. Orphan deletion is scoped to the `(entity_id, rule_type)` pairs the
//! config still declares, so dropping a band from the YAML retires exactly that
//! band and leaves rows another writer owns untouched.
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

fn tally(report: &mut IngestReport, outcome: UpsertOutcome) {
    match outcome {
        UpsertOutcome::Inserted => report.inserted += 1,
        UpsertOutcome::Updated => report.updated += 1,
        UpsertOutcome::Skipped => report.skipped += 1,
    }
}

impl AccessControlIngestionService {
    pub async fn ingest_marketplace_access(
        &self,
        marketplaces: &HashMap<MarketplaceId, MarketplaceConfig>,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        // Why: every rule type is validated before the transaction opens, so a
        // malformed slug cannot leave half a marketplace's grants written.
        for (id, cfg) in marketplaces {
            for rule in &cfg.access.rules {
                RuleType::extension(rule.rule_type.clone()).map_err(|_| {
                    AuthzError::Validation(format!(
                        "marketplace '{}': access.rules rule_type '{}' is not a valid subject \
                         dimension",
                        id.as_str(),
                        rule.rule_type
                    ))
                })?;
            }
        }

        let mut tx = self.write_pool.begin().await?;
        let mut report = IngestReport::default();

        let bands = declared_bands(marketplaces);
        if options.delete_orphans && !bands.entity_ids.is_empty() {
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
            .execute(&mut *tx)
            .await?;
            report.deleted = res.rows_affected() as usize;
        }

        for (id, cfg) in marketplaces {
            if !cfg.access.declares_rules() {
                continue;
            }
            let entity_id = id.as_str();
            upsert_marketplace_entity_row(&mut tx, entity_id, cfg.access.default_included).await?;

            for role in &cfg.access.roles {
                let target = Target {
                    entity_kind: EntityKind::Marketplace,
                    entity_id,
                    rule_type: RuleType::ROLE,
                    rule_value: role.as_str(),
                    access: "allow",
                    justification: cfg.access.justification.as_deref(),
                };
                let outcome = upsert_target(&mut tx, &target, options.override_existing).await?;
                tally(&mut report, outcome);
            }

            for rule in &cfg.access.rules {
                let rule_type = RuleType::extension(rule.rule_type.clone())
                    .map_err(|e| AuthzError::Validation(e.to_string()))?;
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
                    let outcome =
                        upsert_target(&mut tx, &target, options.override_existing).await?;
                    tally(&mut report, outcome);
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
