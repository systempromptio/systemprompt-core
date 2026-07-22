//! Inventory-registered fixture extension that makes the asset-validation
//! arms of `DeployArtifacts` reachable in this test binary. The declared
//! assets are gated behind `SYNC_COV_ASSET_MODE` so that, by default, the
//! extension declares nothing and the rest of the suite keeps its
//! "no asset extensions" premise; nextest's process-per-test isolation lets
//! individual tests opt in to a mode.

use systemprompt_extension::{
    AssetDefinition, AssetPaths, AssetType, Extension, ExtensionMetadata, register_extension,
};

pub const MODE_ENV: &str = "SYNC_COV_ASSET_MODE";

#[derive(Default)]
pub struct CovAssetExtension;

impl Extension for CovAssetExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covsyncassets",
            name: "Coverage Asset Extension",
            version: "0.0.1",
        }
    }

    fn declares_assets(&self) -> bool {
        std::env::var(MODE_ENV).is_ok()
    }

    fn required_assets(&self, paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        match std::env::var(MODE_ENV).as_deref() {
            Ok("required") => vec![
                AssetDefinition::css(paths.storage_files().join("cov-required.css"), "cov.css"),
                AssetDefinition::builder(
                    paths.web_dist().join("cov-optional.js"),
                    "cov-optional.js",
                    AssetType::JavaScript,
                )
                .optional()
                .build(),
            ],
            Ok("outside") => vec![AssetDefinition::html(
                std::env::temp_dir().join("sync-cov-outside.html"),
                "outside.html",
            )],
            _ => vec![],
        }
    }
}

register_extension!(CovAssetExtension);
