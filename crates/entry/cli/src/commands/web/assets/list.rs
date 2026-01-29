use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use std::path::Path;
use walkdir::WalkDir;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::super::paths::WebPaths;
use super::super::types::{AssetListOutput, AssetSummary, AssetType};

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

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<AssetListOutput>> {
    let web_paths = WebPaths::resolve()?;
    let assets_dir = &web_paths.assets;

    if !assets_dir.exists() {
        return Ok(CommandResult::table(AssetListOutput { assets: vec![] })
            .with_title("Assets")
            .with_columns(vec![
                "path".to_string(),
                "asset_type".to_string(),
                "size_bytes".to_string(),
                "modified".to_string(),
            ]));
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
            || "unknown".to_string(),
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

    let output = AssetListOutput { assets };

    Ok(CommandResult::table(output)
        .with_title("Assets")
        .with_columns(vec![
            "path".to_string(),
            "asset_type".to_string(),
            "size_bytes".to_string(),
            "modified".to_string(),
        ]))
}

fn determine_asset_type(path: &Path, relative_path: &str) -> AssetType {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if filename.starts_with("favicon") {
        return AssetType::Favicon;
    }

    if relative_path.starts_with("logos/") || filename.contains("logo") {
        return AssetType::Logo;
    }

    match extension.as_str() {
        "css" => AssetType::Css,
        "ttf" | "woff" | "woff2" | "otf" | "eot" => AssetType::Font,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" => AssetType::Image,
        _ => AssetType::Other,
    }
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
