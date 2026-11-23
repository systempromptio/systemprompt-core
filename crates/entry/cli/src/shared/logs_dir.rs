//! The logs directory the `admin agents logs` and `plugins mcp logs` disk
//! modes read: the profile's logs root, or a `--logs-dir` confined to it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::{Context, Result};
use systemprompt_config::paths::AppPaths;
use systemprompt_models::path_confine::logs::confined_logs_dir;

use crate::context::CommandContext;

pub fn resolve_logs_dir(ctx: &CommandContext, requested: Option<&str>) -> Result<PathBuf> {
    let boot = ctx.boot()?;
    let profile = boot.profile();
    let paths = AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        boot.services_root().map(|root| root.path.as_path()),
    )
    .context("the profile's paths do not resolve to a logs root")?;
    Ok(confined_logs_dir(&paths.system().logs(), requested)?)
}
