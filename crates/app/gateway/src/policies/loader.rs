//! Filesystem entry point for gateway-policy bootstrap.
//!
//! Reads `services/gateway/policies.yaml` and projects it into
//! `ai_gateway_policies`. Mirrors the access-control YAML loader: the YAML is
//! the version-controlled source of truth, ingested at every server boot.
//! A missing file is a no-op (an instance simply runs with no policies, i.e.
//! permissive).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::config::GatewayPolicyConfig;
use super::error::GatewayPolicyError;
use super::ingestion::{GatewayPolicyIngestionService, IngestOptions, IngestReport};
use systemprompt_ai::repository::AiGatewayPolicyRepository;

pub const GATEWAY_POLICIES_FILE: &str = "gateway/policies.yaml";

pub async fn load_from_yaml(
    repository: &AiGatewayPolicyRepository,
    services_path: &Path,
) -> Result<IngestReport, GatewayPolicyError> {
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
            return Err(GatewayPolicyError::Read { path, source: err });
        },
    };

    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&content)
        .map_err(|source| GatewayPolicyError::Parse { path, source })?;

    let service = GatewayPolicyIngestionService::from_repository(repository.clone());
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
