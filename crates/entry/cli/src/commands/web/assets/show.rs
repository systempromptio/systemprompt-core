use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use clap::Args;
use std::fs;

use crate::CliConfig;
use crate::shared::CommandResult;
use systemprompt_config::ProfileBootstrap;

use super::super::paths::WebPaths;
use super::super::types::AssetDetailOutput;
use super::asset_type::determine_asset_type;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "Asset path (relative to assets directory)")]
    pub path: String,
}

pub fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandResult<AssetDetailOutput>> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let web_paths = WebPaths::resolve()?;
    let assets_dir = &web_paths.assets;
    let asset_path = assets_dir.join(&args.path);

    if !asset_path.exists() {
        return Err(anyhow!("Asset '{}' not found", args.path));
    }

    if !asset_path.is_file() {
        return Err(anyhow!("'{}' is not a file", args.path));
    }

    let metadata = asset_path
        .metadata()
        .context("Failed to get file metadata")?;
    let size_bytes = metadata.len();
    let modified = metadata.modified().ok().map_or_else(
        || "unknown".to_string(),
        |t| {
            let datetime: DateTime<Utc> = t.into();
            datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
        },
    );

    let asset_type = determine_asset_type(&asset_path, &args.path);
    let referenced_in = find_config_references(&args.path, profile);

    let output = AssetDetailOutput {
        path: args.path.clone(),
        absolute_path: asset_path.to_string_lossy().to_string(),
        asset_type,
        size_bytes,
        modified,
        referenced_in,
    };

    Ok(CommandResult::card(output).with_title(format!("Asset: {}", args.path)))
}

fn find_config_references(asset_path: &str, profile: &systemprompt_models::Profile) -> Vec<String> {
    let mut references = Vec::new();

    let web_config_path = profile.paths.web_config();
    if let Ok(content) = fs::read_to_string(&web_config_path) {
        let search_patterns = [
            format!("/assets/{}", asset_path),
            format!("assets/{}", asset_path),
            asset_path.to_string(),
        ];

        for pattern in &search_patterns {
            if content.contains(pattern) {
                references.push(format!("web config: {}", web_config_path));
                break;
            }
        }
    }

    let metadata_path = profile.paths.web_metadata();
    if let Ok(content) = fs::read_to_string(&metadata_path) {
        let search_patterns = [
            format!("/assets/{}", asset_path),
            format!("assets/{}", asset_path),
            asset_path.to_string(),
        ];

        for pattern in &search_patterns {
            if content.contains(pattern) {
                references.push(format!("metadata: {}", metadata_path));
                break;
            }
        }
    }

    references
}
