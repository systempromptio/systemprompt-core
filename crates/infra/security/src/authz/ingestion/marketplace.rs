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
//! still declares, narrowed further to rows this pass's own `source` wrote and
//! to the marketplaces the caller's [`IngestScope`](super::IngestScope) claims.
//! Within a declared band the rows are pruned to exactly what the config names;
//! a band the config no longer mentions is left in place, because a delete
//! scoped by entity alone would take rules another writer owns
//! with it. Removing a whole band from the YAML therefore does not revoke it —
//! that is a deliberate deletion, made with the CLI or by hand.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::MarketplaceConfig;

use super::super::error::{AuthzError, AuthzResult};
use super::super::types::{EntityKind, RuleType};
use super::subjects::{SubjectMention, find_unknown_subjects};
use super::upsert::{Target, upsert_marketplace_entity_row, upsert_target};
use super::{AccessControlIngestionService, IngestOptions, IngestReport, tally};

struct DeclaredBands {
    entity_ids: Vec<String>,
    rule_types: Vec<String>,
}

fn declared_bands(
    marketplaces: &HashMap<MarketplaceId, MarketplaceConfig>,
    options: &IngestOptions,
) -> DeclaredBands {
    let mut entity_ids = Vec::new();
    let mut rule_types = Vec::new();
    for (id, cfg) in marketplaces {
        if !options.scope.owns(EntityKind::Marketplace, id.as_str()) {
            continue;
        }
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

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

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

async fn delete_declared_bands(
    tx: &mut Tx<'_>,
    bands: &DeclaredBands,
    source: &str,
) -> AuthzResult<usize> {
    let res = sqlx::query!(
        r#"
        DELETE FROM access_control_rules
        WHERE entity_type = 'marketplace'
          AND source = $3
          AND (entity_id, rule_type) IN (
              SELECT * FROM UNNEST($1::text[], $2::text[])
          )
        "#,
        &bands.entity_ids,
        &bands.rule_types,
        source,
    )
    .execute(&mut **tx)
    .await?;
    Ok(res.rows_affected() as usize)
}

async fn upsert_marketplace(
    tx: &mut Tx<'_>,
    entity_id: &str,
    cfg: &MarketplaceConfig,
    options: &IngestOptions,
    mentions: &mut BTreeSet<SubjectMention>,
) -> AuthzResult<IngestReport> {
    let mut report = IngestReport::default();
    upsert_marketplace_entity_row(tx, entity_id, cfg.access.default_included).await?;

    for role in &cfg.access.roles {
        let target = Target {
            entity_kind: EntityKind::Marketplace,
            entity_id,
            rule_type: RuleType::ROLE,
            rule_value: role.as_str(),
            access: "allow",
            justification: cfg.access.justification.as_deref(),
            source: &options.source,
        };
        let outcome = upsert_target(tx, &target, options.override_existing).await?;
        tally(&mut report, outcome);
        mentions.insert(SubjectMention {
            rule_type: RuleType::ROLE.to_string(),
            value: role.as_str().to_owned(),
            entity: format!("marketplace:{entity_id}"),
        });
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
                source: &options.source,
            };
            let outcome = upsert_target(tx, &target, options.override_existing).await?;
            tally(&mut report, outcome);
            mentions.insert(SubjectMention {
                rule_type: rule_type.to_string(),
                value: value.as_str().to_owned(),
                entity: format!("marketplace:{entity_id}"),
            });
        }
    }

    Ok(report)
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

        let bands = declared_bands(marketplaces, &options);
        if options.delete_orphans && !bands.entity_ids.is_empty() {
            report.deleted = delete_declared_bands(&mut tx, &bands, &options.source).await?;
        }

        let mut mentions = BTreeSet::new();
        for (id, cfg) in marketplaces {
            if !cfg.access.declares_rules() {
                continue;
            }
            let one =
                upsert_marketplace(&mut tx, id.as_str(), cfg, &options, &mut mentions).await?;
            report.inserted += one.inserted;
            report.updated += one.updated;
            report.skipped += one.skipped;
            report.protected += one.protected;
        }

        report.unknown_subjects = find_unknown_subjects(&mut tx, &mentions).await?;

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_marketplace_access_loaded",
            inserted = report.inserted,
            updated = report.updated,
            skipped = report.skipped,
            deleted = report.deleted,
            protected = report.protected,
            unknown_subjects = report.unknown_subjects.len(),
            source = %options.source,
            override_existing = options.override_existing,
            delete_orphans = options.delete_orphans,
            "marketplace access blocks ingested",
        );

        Ok(report)
    }
}
