//! Seeding the model catalog and pointing the gateway default at a provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_logging::CliService;

use super::templates::get_services_path;
use crate::cli::init::templates::{ai_config, catalog_files};

pub fn update_ai_config_default_provider(provider: &str) -> Result<()> {
    let services_path = get_services_path()?;
    let ai_dir = Path::new(&services_path).join("ai");
    let gateway_path = ai_dir.join("gateway.yaml");

    if !gateway_path.exists() {
        CliService::warning("Model catalog not found. Creating services/ai/");
        CliService::info("Run 'systemprompt cloud init' for full project setup");
        std::fs::create_dir_all(&ai_dir)
            .with_context(|| format!("Failed to create directory {}", ai_dir.display()))?;
        if !ai_dir.join("config.yaml").exists() {
            std::fs::write(ai_dir.join("config.yaml"), ai_config())
                .with_context(|| format!("Failed to write {}", ai_dir.display()))?;
        }
        for (name, body) in catalog_files(provider)? {
            let path = ai_dir.join(name);
            std::fs::write(&path, body)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
        CliService::success(&format!(
            "Created the model catalog in {}",
            ai_dir.display()
        ));
        return Ok(());
    }

    let mut document = systemprompt_config::read_catalog(&ai_dir)
        .with_context(|| format!("Failed to read {}", ai_dir.display()))?;
    let Some(model) = systemprompt_cloud::provider_catalog::first_model_on(&document, provider)
    else {
        CliService::warning(&format!(
            "The catalog deploys no model on '{provider}'; gateway.default_model is unchanged"
        ));
        return Ok(());
    };
    let gateway = document
        .gateway
        .get_or_insert_with(systemprompt_manifest::services::GatewayConfigSpec::default);
    gateway.default_model = Some(model.clone());
    systemprompt_cloud::provider_catalog::write_gateway(&ai_dir, Some(gateway))?;
    CliService::success(&format!(
        "Set gateway.default_model to '{model}' (served by '{provider}')"
    ));
    Ok(())
}
