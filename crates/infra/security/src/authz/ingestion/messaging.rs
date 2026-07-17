//! Projection of each chat-platform app's `authz.allowed_roles` into the
//! workspace/tenant-scoped authz rows, reusing the role-rule upsert path that
//! [`super::AccessControlIngestionService::ingest_config`] uses for YAML rules.
//!
//! A from-scratch deploy authorizes Slack/Teams purely from
//! `services/*.yaml` — a hand-written `roles.yaml` is needed only for finer
//! per-channel control. Mirrors the marketplace-access ingestion exactly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_models::services::{SlackAppConfig, TeamsAppConfig};

use super::super::error::AuthzResult;
use super::super::types::{EntityKind, RuleType};
use super::upsert::{Target, UpsertOutcome, upsert_entity_row, upsert_target};
use super::{AccessControlIngestionService, IngestOptions, IngestReport};

/// One workspace/tenant entity to seed: its id and the roles allowed to drive
/// it.
struct AppSeed {
    entity_id: String,
    roles: Vec<String>,
}

impl AccessControlIngestionService {
    /// Seed `slack_workspace` allow-rules from each enabled Slack app's
    /// `authz.allowed_roles`.
    pub async fn ingest_slack_apps(
        &self,
        apps: &HashMap<String, SlackAppConfig>,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        let seeds = apps
            .values()
            .filter(|app| app.enabled && !app.authz.allowed_roles.is_empty())
            .map(|app| AppSeed {
                entity_id: app.workspace_id.as_str().to_owned(),
                roles: app.authz.allowed_roles.clone(),
            })
            .collect();
        self.ingest_app_seeds(seeds, EntityKind::SlackWorkspace, "slack", options)
            .await
    }

    /// Seed `teams_tenant` allow-rules from each enabled Teams app's
    /// `authz.allowed_roles`.
    pub async fn ingest_teams_apps(
        &self,
        apps: &HashMap<String, TeamsAppConfig>,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        let seeds = apps
            .values()
            .filter(|app| app.enabled && !app.authz.allowed_roles.is_empty())
            .map(|app| AppSeed {
                entity_id: app.tenant_id.as_str().to_owned(),
                roles: app.authz.allowed_roles.clone(),
            })
            .collect();
        self.ingest_app_seeds(seeds, EntityKind::TeamsTenant, "teams", options)
            .await
    }

    async fn ingest_app_seeds(
        &self,
        seeds: Vec<AppSeed>,
        kind: EntityKind,
        source_prefix: &str,
        options: IngestOptions,
    ) -> AuthzResult<IngestReport> {
        let mut tx = self.write_pool.begin().await?;
        let mut report = IngestReport::default();

        let ingested_ids: Vec<String> = seeds.iter().map(|s| s.entity_id.clone()).collect();

        if options.delete_orphans && !ingested_ids.is_empty() {
            // Why: scope the sweep to the apps this pass owns, mirroring the
            // marketplace path; an unscoped delete would race other writers
            // holding role rules under a different entity.
            let res = sqlx::query!(
                r#"
                DELETE FROM access_control_rules
                WHERE rule_type = 'role'
                  AND entity_type = $1
                  AND entity_id = ANY($2::text[])
                "#,
                kind.as_str(),
                &ingested_ids,
            )
            .execute(&mut *tx)
            .await?;
            report.deleted = res.rows_affected() as usize;
        }

        for seed in &seeds {
            let source = format!("{source_prefix}:{}", seed.entity_id);
            upsert_entity_row(&mut tx, kind, &seed.entity_id, false, &source).await?;
            for role in &seed.roles {
                let target = Target {
                    entity_kind: kind,
                    entity_id: &seed.entity_id,
                    rule_type: RuleType::Role,
                    rule_value: role.as_str(),
                    access: "allow",
                    justification: None,
                };
                match upsert_target(&mut tx, &target, options.override_existing).await? {
                    UpsertOutcome::Inserted => report.inserted += 1,
                    UpsertOutcome::Updated => report.updated += 1,
                    UpsertOutcome::Skipped => report.skipped += 1,
                }
            }
        }

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_messaging_access_loaded",
            platform = source_prefix,
            inserted = report.inserted,
            updated = report.updated,
            skipped = report.skipped,
            deleted = report.deleted,
            "messaging app authz seeds ingested",
        );

        Ok(report)
    }
}
