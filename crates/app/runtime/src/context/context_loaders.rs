//! Helpers that load optional ancillary state into [`AppContext`].
//!
//! These run during [`crate::AppContextBuilder::build`] but are split
//! out of `context.rs` to keep the main type definition under 300
//! lines. A missing or unreadable content config degrades gracefully with a
//! CLI warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_logging::CliService;
use systemprompt_manifest::Config;
use systemprompt_models::ContentConfigRaw;

pub(crate) fn load_content_config(
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
