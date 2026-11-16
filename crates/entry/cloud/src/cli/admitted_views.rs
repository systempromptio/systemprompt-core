//! The open admitted views the cloud preflight checks read from the host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_extension::cli::CommandHost;
use systemprompt_extension::{AdmittedAssets, AdmittedConfigs, AdmittedMigrations};

/// The admitted composition's declared migrations, configuration sections
/// and assets, as the kernel seeds them for a member's command.
#[derive(Debug, Clone)]
pub struct AdmittedViews {
    pub migrations: AdmittedMigrations,
    pub configs: AdmittedConfigs,
    pub assets: AdmittedAssets,
}

impl AdmittedViews {
    pub async fn resolve(host: &dyn CommandHost) -> Result<Self> {
        let resolved = host.resolved().await?;
        Ok(Self {
            migrations: resolved.get::<AdmittedMigrations>()?,
            configs: resolved.get::<AdmittedConfigs>()?,
            assets: resolved.get::<AdmittedAssets>()?,
        })
    }
}
