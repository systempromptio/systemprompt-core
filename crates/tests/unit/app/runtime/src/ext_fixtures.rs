//! Inventory-registered fixture extensions that make the extension arms of
//! the startup validator reachable in this test binary:
//! - `covextok`: config-bearing, always validates OK
//! - `covextbad`: config-bearing, always rejects its config
//! - `covassets_missing`: declares a required asset that never exists
//! - `covassets_ok`: declares a required asset the boot fixture creates
//!
//! Registration is per-binary (inventory), so every test in this crate that
//! reaches extension discovery sees all four; assertions account for that.

use std::sync::Arc;

use systemprompt_extension::{
    AssetDefinition, AssetPaths, ConfigError, Extension, ExtensionMetadata, register_extension,
};
use systemprompt_marketplace::{
    MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError, register_marketplace_filter,
};

#[derive(Default)]
pub struct CovExtOk;

impl Extension for CovExtOk {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covextok",
            name: "Coverage OK Extension",
            version: "0.0.1",
        }
    }

    fn config_prefix(&self) -> Option<&str> {
        Some("covextok")
    }
}

#[derive(Default)]
pub struct CovExtBad;

impl Extension for CovExtBad {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covextbad",
            name: "Coverage Bad Extension",
            version: "0.0.1",
        }
    }

    fn config_prefix(&self) -> Option<&str> {
        Some("covextbad")
    }

    fn validate_config(&self, _config: &serde_json::Value) -> Result<(), ConfigError> {
        Err(ConfigError::InvalidValue {
            key: "covextbad.mode".to_owned(),
            message: "fixture always rejects".to_owned(),
        })
    }
}

#[derive(Default)]
pub struct CovAssetsMissing;

impl Extension for CovAssetsMissing {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covassets_missing",
            name: "Coverage Missing Assets Extension",
            version: "0.0.1",
        }
    }

    fn declares_assets(&self) -> bool {
        true
    }

    fn required_assets(&self, paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        vec![AssetDefinition::css(
            paths.storage_files().join("covassets_missing/absent.css"),
            "css/absent.css",
        )]
    }
}

#[derive(Default)]
pub struct CovAssetsOk;

impl Extension for CovAssetsOk {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covassets_ok",
            name: "Coverage Present Assets Extension",
            version: "0.0.1",
        }
    }

    fn declares_assets(&self) -> bool {
        true
    }

    fn required_assets(&self, paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        vec![AssetDefinition::css(
            paths.storage_files().join("covassets_ok/present.css"),
            "css/present.css",
        )]
    }
}

#[derive(Debug)]
pub struct CovMarketplaceFilter;

#[async_trait::async_trait]
impl MarketplaceFilter for CovMarketplaceFilter {
    async fn filter(
        &self,
        _user_id: &systemprompt_identifiers::UserId,
        candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        Ok(candidate)
    }
}

fn failing_filter_factory(
    _pool: &systemprompt_database::DbPool,
) -> Result<Arc<dyn MarketplaceFilter>, MarketplaceFilterError> {
    Err(MarketplaceFilterError::Backend(
        "fixture factory always fails".to_owned(),
    ))
}

fn cov_filter_factory(
    _pool: &systemprompt_database::DbPool,
) -> Result<Arc<dyn MarketplaceFilter>, MarketplaceFilterError> {
    Ok(Arc::new(CovMarketplaceFilter))
}

register_marketplace_filter!(failing_filter_factory, priority = 100);
register_marketplace_filter!(cov_filter_factory, priority = 10);

register_extension!(CovExtOk);
register_extension!(CovExtBad);
register_extension!(CovAssetsMissing);
register_extension!(CovAssetsOk);
