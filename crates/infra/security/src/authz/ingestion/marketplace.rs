//! Projection of each marketplace's declarative `access` block into the
//! marketplace-scoped authz rows, reusing the role-rule upsert path that
//! [`super::AccessControlIngestionService::ingest_config`] uses for YAML rules.

use std::collections::HashMap;

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::MarketplaceConfig;

use super::super::error::AuthzResult;
use super::super::types::{EntityKind, RuleType};
use super::upsert::{Target, UpsertOutcome, upsert_marketplace_entity_row, upsert_target};
use super::{AccessControlIngestionService, IngestOptions, IngestReport};

impl AccessControlIngestionService {
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
