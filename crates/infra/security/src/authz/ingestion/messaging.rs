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

use std::collections::{BTreeSet, HashMap};

use systemprompt_models::services::{SlackAppConfig, TeamsAppConfig};

use super::super::error::AuthzResult;
use super::super::types::{EntityKind, RuleType};
use super::subjects::{SubjectMention, find_unknown_subjects};
use super::upsert::{Target, upsert_entity_row, upsert_target};
use super::{AccessControlIngestionService, IngestOptions, IngestReport, tally};

struct AppSeed {
    entity_id: String,
    roles: Vec<String>,
}

impl AccessControlIngestionService {
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

        let ingested_ids: Vec<String> = seeds
            .iter()
            .filter(|s| options.scope.owns(kind, &s.entity_id))
            .map(|s| s.entity_id.clone())
            .collect();

        if options.delete_orphans && !ingested_ids.is_empty() {
            let res = sqlx::query!(
                r#"
                DELETE FROM access_control_rules
                WHERE rule_type = 'role'
                  AND entity_type = $1
                  AND source = $3
                  AND entity_id = ANY($2::text[])
                "#,
                kind.as_str(),
                &ingested_ids,
                options.source,
            )
            .execute(&mut *tx)
            .await?;
            report.deleted = res.rows_affected() as usize;
        }

        let mut mentions = BTreeSet::new();
        for seed in &seeds {
            let source = format!("{source_prefix}:{}", seed.entity_id);
            upsert_entity_row(&mut tx, kind, &seed.entity_id, false, &source).await?;
            for role in &seed.roles {
                let target = Target {
                    entity_kind: kind,
                    entity_id: &seed.entity_id,
                    rule_type: RuleType::ROLE,
                    rule_value: role.as_str(),
                    access: "allow",
                    justification: None,
                    source: &options.source,
                };
                let outcome = upsert_target(&mut tx, &target, options.override_existing).await?;
                tally(&mut report, outcome);
                mentions.insert(SubjectMention {
                    rule_type: RuleType::ROLE.to_string(),
                    value: role.clone(),
                    entity: format!("{}:{}", kind.as_str(), seed.entity_id),
                });
            }
        }

        report.unknown_subjects = find_unknown_subjects(&mut tx, &mentions).await?;

        tx.commit().await?;

        tracing::info!(
            target = "bootstrap_messaging_access_loaded",
            platform = source_prefix,
            inserted = report.inserted,
            updated = report.updated,
            skipped = report.skipped,
            deleted = report.deleted,
            protected = report.protected,
            unknown_subjects = report.unknown_subjects.len(),
            source = %options.source,
            "messaging app authz seeds ingested",
        );

        Ok(report)
    }
}
