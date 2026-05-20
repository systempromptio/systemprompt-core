//! Bootstrap-time projection of [`GatewayPolicyConfig`] into
//! `ai_gateway_policies`.
//!
//! This mirrors `systemprompt_security::authz::AccessControlIngestionService`:
//! a domain object parsed from disk is upserted into a typed repository, with
//! explicit `override_existing` and `delete_orphans` knobs. Direction is fixed
//! (YAML → DB).

use std::collections::HashSet;

use systemprompt_database::DbPool;

use super::config::GatewayPolicyConfig;
use crate::error::RepositoryError;
use crate::repository::AiGatewayPolicyRepository;

/// Ingestion knobs, matching `AccessControlIngestionService::IngestOptions`.
#[derive(Debug, Clone, Copy, Default)]
pub struct IngestOptions {
    /// When `false`, an already-present policy is left untouched.
    pub override_existing: bool,
    /// When `true`, policies in the DB but absent from the config are deleted.
    pub delete_orphans: bool,
}

/// Counts surfaced to the publish-pipeline log.
#[derive(Debug, Clone, Copy, Default)]
pub struct IngestReport {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
}

/// Projects a [`GatewayPolicyConfig`] into the `ai_gateway_policies` table.
#[derive(Debug, Clone)]
pub struct GatewayPolicyIngestionService {
    repo: AiGatewayPolicyRepository,
}

impl GatewayPolicyIngestionService {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        Ok(Self {
            repo: AiGatewayPolicyRepository::new(db)?,
        })
    }

    pub const fn from_repository(repo: AiGatewayPolicyRepository) -> Self {
        Self { repo }
    }

    pub async fn ingest_config(
        &self,
        cfg: &GatewayPolicyConfig,
        options: IngestOptions,
    ) -> Result<IngestReport, RepositoryError> {
        cfg.validate()?;

        let existing: HashSet<String> = self.repo.list_all_names().await?.into_iter().collect();
        let mut declared: HashSet<&str> = HashSet::with_capacity(cfg.policies.len());
        let mut report = IngestReport::default();

        for entry in &cfg.policies {
            declared.insert(entry.name.as_str());
            let already_present = existing.contains(&entry.name);
            if already_present && !options.override_existing {
                report.skipped += 1;
                continue;
            }
            let spec =
                serde_json::to_value(&entry.spec).map_err(|err| RepositoryError::InvalidData {
                    field: format!("policies.{}.spec", entry.name),
                    reason: err.to_string(),
                })?;
            self.repo.upsert(&entry.name, &spec, entry.enabled).await?;
            if already_present {
                report.updated += 1;
            } else {
                report.inserted += 1;
            }
        }

        if options.delete_orphans {
            for name in &existing {
                if !declared.contains(name.as_str()) {
                    self.repo.delete_by_name(name).await?;
                    report.deleted += 1;
                }
            }
        }

        tracing::info!(
            target: "bootstrap_gateway_policy_loaded",
            policies_inserted = report.inserted,
            policies_updated = report.updated,
            policies_skipped = report.skipped,
            policies_deleted = report.deleted,
            override_existing = options.override_existing,
            delete_orphans = options.delete_orphans,
            "gateway-policy YAML ingested",
        );

        Ok(report)
    }
}
