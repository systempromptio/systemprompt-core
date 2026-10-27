//! Each theme a marketplace's `opencode:` block declares must be shipped, with
//! its pinned digest, by one of the marketplace's plugins under
//! `hosts/opencode/themes/<name>.json`. The bridge skips a theme it cannot
//! match on the user's machine; the import refuses it instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_manifest::services::OpenCodeMarketplaceConfig;

use crate::error::MarketplaceError;

const THEMES_DIR: &str = "hosts/opencode/themes";

pub(super) struct ThemePins<'a> {
    unmatched: Vec<&'a str>,
    config: Option<&'a OpenCodeMarketplaceConfig>,
}

impl<'a> ThemePins<'a> {
    pub(super) fn new(config: Option<&'a OpenCodeMarketplaceConfig>) -> Self {
        let unmatched = config
            .map(|c| c.themes.iter().map(|t| t.name.as_str()).collect())
            .unwrap_or_default();
        Self { unmatched, config }
    }

    pub(super) fn scan(&mut self, plugin_dir: &Path) -> Result<(), MarketplaceError> {
        let Some(config) = self.config else {
            return Ok(());
        };
        let mut matched = BTreeSet::new();
        for theme in config
            .themes
            .iter()
            .filter(|t| self.unmatched.contains(&t.name.as_str()))
        {
            let path = plugin_dir
                .join(THEMES_DIR)
                .join(format!("{}.json", theme.name));
            if !path.is_file() {
                continue;
            }
            let bytes =
                std::fs::read(&path).map_err(|e| MarketplaceError::import(&path, "read", e))?;
            if hex::encode(Sha256::digest(&bytes)) == theme.sha256.as_str() {
                matched.insert(theme.name.as_str());
            }
        }
        self.unmatched.retain(|name| !matched.contains(name));
        Ok(())
    }

    pub(super) fn finish(self, manifest_path: &Path) -> Result<(), MarketplaceError> {
        if self.unmatched.is_empty() {
            return Ok(());
        }
        Err(MarketplaceError::Import {
            path: manifest_path.display().to_string(),
            message: format!(
                "no plugin ships {THEMES_DIR}/<name>.json with the pinned sha256 for opencode \
                 theme(s): {}",
                self.unmatched.join(", ")
            ),
        })
    }
}
