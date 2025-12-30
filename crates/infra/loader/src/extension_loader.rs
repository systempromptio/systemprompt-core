//! Extension discovery and loading.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use systemprompt_models::{DiscoveredExtension, ExtensionManifest};

#[derive(Debug, Clone, Copy)]
pub struct ExtensionLoader;

impl ExtensionLoader {
    pub fn discover(project_root: &Path) -> Result<Vec<DiscoveredExtension>> {
        let extensions_dir = project_root.join("extensions");

        if !extensions_dir.exists() {
            return Ok(vec![]);
        }

        let mut discovered = vec![];

        Self::scan_directory(&extensions_dir, &mut discovered)?;

        if let Ok(entries) = fs::read_dir(&extensions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::scan_directory(&path, &mut discovered)?;
                }
            }
        }

        Ok(discovered)
    }

    fn scan_directory(dir: &Path, discovered: &mut Vec<DiscoveredExtension>) -> Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let ext_dir = entry.path();
            if !ext_dir.is_dir() {
                continue;
            }

            let manifest_path = ext_dir.join("manifest.yaml");
            if manifest_path.exists() {
                match Self::load_manifest(&manifest_path) {
                    Ok(manifest) => {
                        discovered.push(DiscoveredExtension::new(
                            manifest,
                            ext_dir,
                            manifest_path,
                        ));
                    },
                    Err(e) => {
                        tracing::warn!(
                            path = %manifest_path.display(),
                            error = %e,
                            "Failed to parse extension manifest, skipping"
                        );
                    },
                }
            }
        }

        Ok(())
    }

    fn load_manifest(path: &Path) -> Result<ExtensionManifest> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest: {}", path.display()))?;

        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse manifest: {}", path.display()))
    }

    pub fn get_enabled_mcp_extensions(project_root: &Path) -> Result<Vec<DiscoveredExtension>> {
        let extensions = Self::discover(project_root)?;
        Ok(extensions
            .into_iter()
            .filter(|e| e.is_mcp() && e.is_enabled())
            .collect())
    }

    pub fn validate_mcp_binaries(
        project_root: &Path,
    ) -> Result<Vec<(String, std::path::PathBuf)>> {
        let extensions = Self::get_enabled_mcp_extensions(project_root)?;
        let target_dir = project_root.join("target/release");

        let mut missing = vec![];

        for ext in extensions {
            if let Some(binary) = ext.binary_name() {
                let binary_path = target_dir.join(binary);
                if !binary_path.exists() {
                    missing.push((binary.to_string(), ext.path.clone()));
                }
            }
        }

        Ok(missing)
    }

    pub fn get_mcp_binary_names(project_root: &Path) -> Result<Vec<String>> {
        let extensions = Self::get_enabled_mcp_extensions(project_root)?;
        Ok(extensions
            .iter()
            .filter_map(|e| e.binary_name().map(String::from))
            .collect())
    }

    pub fn validate(project_root: &Path) -> Result<ExtensionValidationResult> {
        let discovered = Self::discover(project_root)?;
        let missing_binaries = Self::validate_mcp_binaries(project_root)?;

        Ok(ExtensionValidationResult {
            discovered,
            missing_binaries,
            missing_manifests: vec![],
        })
    }
}

#[derive(Debug)]
pub struct ExtensionValidationResult {
    pub discovered: Vec<DiscoveredExtension>,
    pub missing_binaries: Vec<(String, std::path::PathBuf)>,
    pub missing_manifests: Vec<std::path::PathBuf>,
}

impl ExtensionValidationResult {
    pub fn is_valid(&self) -> bool {
        self.missing_binaries.is_empty()
    }

    pub fn format_missing_binaries(&self) -> String {
        self.missing_binaries
            .iter()
            .map(|(binary, path)| format!("  ✗ {} ({})", binary, path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
