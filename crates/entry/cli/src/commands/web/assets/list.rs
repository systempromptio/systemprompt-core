//! `web assets list` command with type filtering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use walkdir::WalkDir;

use crate::CliConfig;
use crate::shared::CommandOutput;

use super::super::paths::WebPaths;
use super::super::types::{AssetSummary, AssetType};
use super::asset_type::determine_asset_type;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum AssetTypeFilter {
    #[default]
    All,
    Css,
    Logo,
    Favicon,
    Font,
    Image,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, value_enum, default_value = "all", help = "Filter by asset type")]
    pub asset_type: AssetTypeFilter,
}

pub(super) fn execute(args: ListArgs, config: &CliConfig) -> Result<CommandOutput> {
    execute_in_dir(args, config, &WebPaths::resolve()?.assets)
}

pub fn execute_in_dir(
    args: ListArgs,
    _config: &CliConfig,
    assets_dir: &std::path::Path,
) -> Result<CommandOutput> {
    if !assets_dir.exists() {
        let empty: Vec<AssetSummary> = vec![];
        return Ok(CommandOutput::table_of(
            vec!["path", "asset_type", "size_bytes", "modified"],
            &empty,
        )
        .with_title("Assets"));
    }

    let mut assets: Vec<AssetSummary> = Vec::new();

    for entry in WalkDir::new(assets_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(assets_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let asset_type = determine_asset_type(path, &relative_path);

        if !matches_filter(asset_type, args.asset_type) {
            continue;
        }

        let metadata = path.metadata().context("Failed to get file metadata")?;
        let size_bytes = metadata.len();
        let modified = metadata.modified().ok().map_or_else(
            || "unknown".to_owned(),
            |t| {
                let datetime: DateTime<Utc> = t.into();
                datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            },
        );

        assets.push(AssetSummary {
            path: relative_path,
            asset_type,
            size_bytes,
            modified,
        });
    }

    assets.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(CommandOutput::table_of(
        vec!["path", "asset_type", "size_bytes", "modified"],
        &assets,
    )
    .with_title("Assets"))
}

fn matches_filter(asset_type: AssetType, filter: AssetTypeFilter) -> bool {
    match filter {
        AssetTypeFilter::All => true,
        AssetTypeFilter::Css => asset_type == AssetType::Css,
        AssetTypeFilter::Logo => asset_type == AssetType::Logo,
        AssetTypeFilter::Favicon => asset_type == AssetType::Favicon,
        AssetTypeFilter::Font => asset_type == AssetType::Font,
        AssetTypeFilter::Image => {
            asset_type == AssetType::Image
                || asset_type == AssetType::Logo
                || asset_type == AssetType::Favicon
        },
    }
}
