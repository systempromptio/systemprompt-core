//! Seeds the provider catalog and gateway routes into the services tree.
//!
//! `admin setup` writes `services/ai/providers.yaml` (the embedded default
//! catalog filtered to the providers whose key was supplied) and
//! `services/ai/gateway.yaml` (one route per provider plus the default), and
//! lists both in the root `includes:`. They are written only when absent — a
//! deployment that already ships its catalog in the image keeps it — unless
//! `--force` asks for a fresh seed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;
use systemprompt_models::services::{GatewayConfigSpec, GatewayState, ProviderRegistry};

use super::catalog;
use super::secrets::SecretsData;
use crate::commands::admin::config::config_section::{
    GATEWAY_FILE_RELATIVE, PROVIDERS_FILE_RELATIVE,
};
use crate::commands::admin::config::services_io::append_include;

#[derive(Serialize)]
struct ProvidersFile {
    providers: ProviderRegistry,
}

#[derive(Serialize)]
struct GatewayFile {
    gateway: GatewayState,
}

pub(super) fn seed(
    services_dir: &Path,
    secrets: &SecretsData,
    default_provider: Option<&ProviderId>,
    force: bool,
) -> Result<()> {
    let registry = catalog::build_registry(secrets);
    let gateway = GatewayState::Spec(GatewayConfigSpec {
        enabled: true,
        routes: catalog::build_routes(secrets),
        default_provider: default_provider.cloned(),
        ..GatewayConfigSpec::default()
    });

    registry
        .validate()
        .context("generated provider registry failed validation")?;
    gateway
        .clone()
        .into_spec()
        .resolve()
        .validate(&registry)
        .context("generated gateway config failed validation")?;

    let providers_header = "# Provider catalog — models, pricing, capabilities, limits.\n# \
                            Implementation configuration shipped with the deployment; \
                            credentials\n# are named by `api_key_secret` and live in the profile \
                            secret store.\n";
    let gateway_header = "# Gateway routes — external model patterns onto providers declared \
                          in\n# providers.yaml. Edit with `systemprompt admin config gateway`.\n";

    write_if_absent(
        services_dir,
        PROVIDERS_FILE_RELATIVE,
        providers_header,
        &ProvidersFile {
            providers: registry,
        },
        force,
    )?;
    write_if_absent(
        services_dir,
        GATEWAY_FILE_RELATIVE,
        gateway_header,
        &GatewayFile { gateway },
        force,
    )?;

    let root = services_dir.join("config").join("config.yaml");
    if root.exists() {
        append_include(&root, PROVIDERS_FILE_RELATIVE)?;
        append_include(&root, GATEWAY_FILE_RELATIVE)?;
    } else {
        CliService::warning(&format!(
            "{} not found; list {PROVIDERS_FILE_RELATIVE} and {GATEWAY_FILE_RELATIVE} in its \
             includes when you create it",
            root.display()
        ));
    }
    Ok(())
}

fn write_if_absent<T: Serialize>(
    services_dir: &Path,
    relative: &str,
    header: &str,
    body: &T,
    force: bool,
) -> Result<()> {
    let path = services_dir.join(relative);
    if path.exists() && !force {
        CliService::info(&format!("Keeping existing {}", path.display()));
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let yaml = serde_yaml::to_string(body).context("Failed to serialize services file")?;
    std::fs::write(&path, format!("{header}{yaml}"))
        .with_context(|| format!("Failed to write {}", path.display()))?;
    CliService::success(&format!("Saved {}", path.display()));
    Ok(())
}
