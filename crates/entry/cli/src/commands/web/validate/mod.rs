//! Validation of the web configuration across config, templates, assets, and
//! sitemap.
//!
//! Runs the per-category validators (selectable via [`ValidationCategory`]) and
//! aggregates their errors and warnings into a single report.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod asset_validation;
mod config_validation;
pub mod sitemap_validation;
pub mod template_validation;

use anyhow::Result;
use clap::{Args, ValueEnum};

use crate::CliConfig;

use super::paths::WebPaths;
use super::types::ValidationOutput;
use systemprompt_loader::BootContext;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ValidationCategory {
    #[default]
    All,
    Config,
    Templates,
    Assets,
    Sitemap,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ValidateArgs {
    #[arg(long, value_enum, help = "Only check specific category")]
    pub only: Option<ValidationCategory>,
}

pub(super) fn execute(
    args: &ValidateArgs,
    _config: &CliConfig,
    boot: &BootContext,
) -> Result<ValidationOutput> {
    let web_paths = WebPaths::resolve(boot)?;
    let profile = boot.profile();

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let category = args.only.unwrap_or(ValidationCategory::All);

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Config
    ) {
        config_validation::validate_config(profile, &web_paths, &mut errors, &mut warnings);
    }

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Templates
    ) {
        template_validation::validate_templates(profile, &web_paths, &mut errors, &mut warnings);
    }

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Assets
    ) {
        asset_validation::validate_assets(profile, &web_paths, &mut errors, &mut warnings);
    }

    if matches!(
        category,
        ValidationCategory::All | ValidationCategory::Sitemap
    ) {
        sitemap_validation::validate_sitemap(profile, &mut errors, &mut warnings);
    }

    let valid = errors.is_empty();
    let items_checked = match category {
        ValidationCategory::All => 4,
        _ => 1,
    };

    Ok(ValidationOutput {
        valid,
        items_checked,
        errors,
        warnings,
    })
}
