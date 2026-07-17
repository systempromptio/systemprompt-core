//! Aggregated extension validation results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::DiscoveredExtension;

#[derive(Debug)]
pub struct ExtensionValidationResult {
    pub discovered: Vec<DiscoveredExtension>,
    pub missing_binaries: Vec<(String, std::path::PathBuf)>,
    pub missing_manifests: Vec<std::path::PathBuf>,
}

impl ExtensionValidationResult {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.missing_binaries.is_empty()
    }

    #[must_use]
    pub fn format_missing_binaries(&self) -> String {
        self.missing_binaries
            .iter()
            .map(|(binary, path)| format!("  ✗ {} ({})", binary, path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
