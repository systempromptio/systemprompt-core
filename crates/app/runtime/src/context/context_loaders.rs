//! Helpers that load optional ancillary state into [`AppContext`].
//!
//! These run during [`crate::AppContextBuilder::build`] but are split
//! out of `context.rs` to keep the main type definition under 300
//! lines. All of them degrade gracefully: when an underlying file is
//! missing or invalid they emit a CLI warning and return `None`, since
//! the affected features (geolocation, landing-page detection) are
//! optional.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_logging::CliService;
use systemprompt_models::{AppPaths, Config, ContentConfigRaw};

#[cfg(feature = "geolocation")]
use systemprompt_analytics::GeoIpReader;

#[cfg(feature = "geolocation")]
pub(super) fn load_geoip_database(config: &Config, show_warnings: bool) -> Option<GeoIpReader> {
    let Some(geoip_path) = &config.geoip_database_path else {
        if show_warnings {
            CliService::warning(
                "GeoIP database not configured - geographic data will not be available",
            );
            CliService::info("  To enable geographic data:");
            CliService::info(
                "  1. Download MaxMind GeoLite2-City database from: https://dev.maxmind.com/geoip/geolite2-free-geolocation-data",
            );
            CliService::info(
                "  2. Add paths.geoip_database to your profile pointing to the .mmdb file",
            );
        }
        return None;
    };

    match maxminddb::Reader::open_readfile(geoip_path) {
        Ok(reader) => Some(Arc::new(reader)),
        Err(e) => {
            if show_warnings {
                CliService::warning(&format!(
                    "Could not load GeoIP database from {geoip_path}: {e}"
                ));
                CliService::info("  Geographic data (country/region/city) will not be available.");
                CliService::info(
                    "  To fix: Ensure the path is correct and the file is a valid MaxMind .mmdb \
                     database",
                );
            }
            None
        },
    }
}

#[cfg(not(feature = "geolocation"))]
#[expect(
    clippy::missing_const_for_fn,
    reason = "mirrors the geolocation loader signature; const would propagate a \
              feature-forked constness to callers"
)]
pub(super) fn load_geoip_database(
    _config: &Config,
    _show_warnings: bool,
) -> Option<systemprompt_analytics::GeoIpReader> {
    None
}

pub(super) fn load_content_config(
    config: &Config,
    app_paths: &AppPaths,
) -> Option<Arc<ContentConfigRaw>> {
    let content_config_path = app_paths.system().content_config().to_path_buf();

    if !content_config_path.exists() {
        CliService::warning(&format!(
            "Content config not found at: {}",
            content_config_path.display()
        ));
        CliService::info("  Landing page detection will not be available.");
        return None;
    }

    let yaml_content = match std::fs::read_to_string(&content_config_path) {
        Ok(c) => c,
        Err(e) => {
            CliService::warning(&format!(
                "Could not read content config from {}: {}",
                content_config_path.display(),
                e
            ));
            CliService::info("  Landing page detection will not be available.");
            return None;
        },
    };

    match serde_yaml::from_str::<ContentConfigRaw>(&yaml_content) {
        Ok(mut content_cfg) => {
            let base_url = config.api_external_url.trim_end_matches('/');

            base_url.clone_into(&mut content_cfg.metadata.structured_data.organization.url);

            let logo = &content_cfg.metadata.structured_data.organization.logo;
            if logo.starts_with('/') {
                content_cfg.metadata.structured_data.organization.logo =
                    format!("{base_url}{logo}");
            }

            Some(Arc::new(content_cfg))
        },
        Err(e) => {
            CliService::warning(&format!(
                "Could not parse content config from {}: {}",
                content_config_path.display(),
                e
            ));
            CliService::info("  Landing page detection will not be available.");
            None
        },
    }
}
