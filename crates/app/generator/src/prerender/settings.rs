//! [`PrerenderSettings`]: what a prerender run reads from the runtime
//! configuration.
//!
//! The runtime `Config` is sealed in the kernel state, so no job context
//! carries it. The generator member reads it once in its provide fn and
//! provides this value, which holds only the web configuration path a
//! prerender run loads; the content and page prerender jobs resolve it from
//! their job context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_manifest::Config;

/// The configuration a prerender run reads, narrowed from the sealed
/// runtime `Config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrerenderSettings {
    web_config_path: PathBuf,
}

impl PrerenderSettings {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            web_config_path: PathBuf::from(&config.web_config_path),
        }
    }

    #[must_use]
    pub fn web_config_path(&self) -> &Path {
        &self.web_config_path
    }
}
