use systemprompt_models::DiscoveredExtension;

/// Outcome of an [`crate::ExtensionLoader::validate`] call.
#[derive(Debug)]
pub struct ExtensionValidationResult {
    /// Every successfully-parsed extension manifest.
    pub discovered: Vec<DiscoveredExtension>,
    /// `(binary_name, manifest_dir)` pairs for binaries that are missing
    /// from the resolved bin directory.
    pub missing_binaries: Vec<(String, std::path::PathBuf)>,
    /// Manifest paths that were referenced but failed to load. Currently
    /// always empty — `discover` swallows manifest-load failures with a
    /// `warn!` log.
    pub missing_manifests: Vec<std::path::PathBuf>,
}

impl ExtensionValidationResult {
    /// Returns `true` if every enabled extension has a usable binary.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.missing_binaries.is_empty()
    }

    /// Renders the missing-binaries list as a human-readable bullet block.
    #[must_use]
    pub fn format_missing_binaries(&self) -> String {
        self.missing_binaries
            .iter()
            .map(|(binary, path)| format!("  ✗ {} ({})", binary, path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
