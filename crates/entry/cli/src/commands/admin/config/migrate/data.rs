//! `config migrate --data`: moving stored data through the legacy map.
//!
//! Reads `services/ai/legacy_map.yaml` (written by the config half), records
//! it in `ai_catalog_legacy_map`, moves personal keys stored under a legacy
//! entry name onto its credential id, backfills `ai_requests.credential` and
//! `endpoint` in batches of [`BATCH`] rows until none is left, and rewrites
//! the 0.72 `anthropic` host-filter tag to the `claude` client family. Every
//! step is idempotent; `ai_requests.provider` keeps its historical values.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use systemprompt_ai::repository::{CatalogMigrationRepository, LegacyMapRow};
use systemprompt_database::{Database, DbPool};
use systemprompt_manifest::Config;
use systemprompt_oauth::repository::BridgeHostPrefsRepository;

use super::legacy_map::{LEGACY_MAP_FILE, LegacyMap};

const BATCH: i64 = 5_000;

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct DataReport {
    pub legacy_entries: usize,
    pub personal_keys_moved: u64,
    pub audit_rows_backfilled: u64,
    pub host_filters_rewritten: u64,
}

pub fn read_map(services: &Path) -> Result<LegacyMap> {
    let path = services.join("ai").join(LEGACY_MAP_FILE);
    let text = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "{} not found; run `systemprompt admin config migrate` first",
            path.display()
        )
    })?;
    serde_yaml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))
}

async fn connect() -> Result<DbPool> {
    let cfg = Config::get()?;
    Ok(Arc::new(
        Database::connect(
            &cfg.database_url,
            cfg.database_write_url.as_deref(),
            &systemprompt_database::PoolConfig::default(),
        )
        .await?,
    ))
}

pub async fn apply(map: &LegacyMap) -> Result<DataReport> {
    let db = connect().await?;
    let repo = CatalogMigrationRepository::new(&db);
    let mut report = DataReport {
        legacy_entries: map.providers.len(),
        ..DataReport::default()
    };
    let routed: Vec<String> = map.providers.keys().cloned().collect();
    for (legacy, target) in &map.providers {
        let row = LegacyMapRow {
            legacy_provider: legacy,
            provider: &target.provider,
            endpoint: &target.endpoint,
            credential: &target.credential,
        };
        repo.upsert_legacy_map(row).await?;
        report.personal_keys_moved += repo
            .update_personal_keys(legacy, &target.credential)
            .await?;
        loop {
            let done = repo.update_audit_batch(row, BATCH).await?;
            report.audit_rows_backfilled += done;
            if done == 0 {
                break;
            }
        }
        loop {
            let done = repo.update_served_audit_batch(row, &routed, BATCH).await?;
            report.audit_rows_backfilled += done;
            if done == 0 {
                break;
            }
        }
    }
    report.host_filters_rewritten = BridgeHostPrefsRepository::new(&db)
        .update_model_protocol_tag("anthropic", "claude")
        .await?;
    Ok(report)
}
