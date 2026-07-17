//! Asset type classification for web asset commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::super::types::AssetType;

pub(super) fn determine_asset_type(path: &Path, relative_path: &str) -> AssetType {
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
