//! Filesystem entry point for gateway-policy bootstrap.
//!
//! Reads `services/gateway/policies.yaml` and projects it into
//! `ai_gateway_policies`. Mirrors the access-control YAML loader: the YAML is
//! the version-controlled source of truth, ingested at every server boot.
//! A missing file is a no-op (an instance simply runs with no policies, i.e.
//! permissive).

use std::path::Path;

use systemprompt_database::DbPool;

use super::config::GatewayPolicyConfig;
use super::ingestion::{GatewayPolicyIngestionService, IngestOptions, IngestReport};
use crate::error::RepositoryError;

pub const GATEWAY_POLICIES_FILE: &str = "gateway/policies.yaml";

/// `override_existing` and `delete_orphans` are both `true`: the YAML is the
/// authoritative source, so every boot reconciles the DB to match it exactly.
pub async fn load_from_yaml(
    db: &DbPool,
    services_path: &Path,
) -> Result<IngestReport, RepositoryError> {
    let path = services_path.join(GATEWAY_POLICIES_FILE);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(
                path = %path.display(),
                "no gateway-policy config — running with no gateway policies"
            );
            return Ok(IngestReport::default());
        },
        Err(err) => {
            return Err(RepositoryError::InvalidData {
                field: GATEWAY_POLICIES_FILE.to_owned(),
                reason: err.to_string(),
            });
        },
    };

    let cfg: GatewayPolicyConfig =
        serde_yaml::from_str(&content).map_err(|err| RepositoryError::InvalidData {
            field: path.display().to_string(),
            reason: err.to_string(),
        })?;

    let service = GatewayPolicyIngestionService::new(db)?;
    service
        .ingest_config(
            &cfg,
            IngestOptions {
                override_existing: true,
                delete_orphans: true,
            },
        )
        .await
}
