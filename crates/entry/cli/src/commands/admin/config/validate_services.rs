//! Services-tree half of `admin config validate`.
//!
//! The command's bootstrap resolves the profile and secrets only. [`prepare`]
//! resolves the services root, the paths and the global config without loading
//! the services tree, and [`load`] then loads and cross-checks that tree,
//! turning a failure into a `services` result entry rather than a bootstrap
//! abort.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_config::paths::AppPaths;
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::ServicesConfig;

use super::types::ConfigFileInfo;
use super::validate::{ValidateArgs, profile_document_target};
use crate::context::CommandContext;

pub async fn prepare(args: &ValidateArgs, ctx: &CommandContext) -> Result<()> {
    if args.schema || profile_document_target(args).is_some() {
        return Ok(());
    }
    let boot = ctx.boot()?;
    let profile = boot.profile();
    let root = boot
        .try_resolve_services_root(env!("CARGO_PKG_VERSION"))
        .await
        .context("Failed to resolve the services bundle sources")?;
    AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        Some(root.path.as_path()),
    )
    .context("Failed to build paths")?;
    boot.try_init_config(Some(root.path.as_path()))
        .context("Failed to initialize configuration")?;
    Ok(())
}

pub(super) fn load(boot: &BootContext) -> (ConfigFileInfo, Option<ServicesConfig>) {
    let path = boot
        .require_services_root()
        .path
        .join("config")
        .join("config.yaml");
    let exists = path.exists();
    let (error, services) = match boot.config_loader().inspecting().load() {
        Ok(services) => (None, Some(services)),
        Err(e) => (Some(format!("{:#}", anyhow::Error::new(e))), None),
    };
    (
        ConfigFileInfo {
            path: path.display().to_string(),
            section: "services".to_owned(),
            exists,
            valid: error.is_none(),
            error,
        },
        services,
    )
}
