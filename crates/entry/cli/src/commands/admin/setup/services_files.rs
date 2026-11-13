//! Seeds the model catalog into the services tree.
//!
//! `admin setup` writes `services/ai/{credentials,providers,models,
//! deployments,gateway}.yaml` — the default catalog narrowed to the vendors
//! whose key was supplied, with the gateway on and its default model on the
//! primary vendor ([`catalog::seed_document`]). The loader reads these files by
//! convention, so nothing is added to `includes:`. They are written only when
//! no catalog file exists yet — a deployment that already ships its catalog in
//! the image keeps it — unless `--force` asks for a fresh seed. A Vertex AI
//! install also gets the `vertex` provider ([`catalog::add_vertex`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_config::write_catalog;
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;
use systemprompt_manifest::services::catalog::CATALOG_FILES;

use crate::shared::provider_catalog::{self as catalog, SeedRequest};
use super::secrets::SecretsData;

pub(super) fn seed(
    services_dir: &Path,
    secrets: &SecretsData,
    default_provider: Option<&ProviderId>,
    force: bool,
) -> Result<()> {
    let dir = services_dir.join("ai");
    let existing = CATALOG_FILES.iter().any(|name| dir.join(name).exists());
    if existing && !force {
        CliService::info(&format!(
            "Keeping the existing catalog in {}",
            dir.display()
        ));
        return Ok(());
    }
    let mut document = catalog::seed_document(SeedRequest {
        present: &secrets.present_providers(),
        primary: default_provider.map(ProviderId::as_str),
        gateway_enabled: true,
    })?;
    if let Some(vertex) = secrets.vertex_seed() {
        catalog::add_vertex(&mut document, vertex)?;
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    write_catalog(&dir, &document).with_context(|| format!("Failed to write {}", dir.display()))?;
    catalog::write_gateway(&dir, document.gateway.as_ref())?;
    CliService::success(&format!("Saved the model catalog in {}", dir.display()));
    Ok(())
}
