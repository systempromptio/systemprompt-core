//! Discovers `extensions/<name>/manifest.yaml` files and resolves the
//! binaries those manifests reference.
//!
//! All operations are infallible at the public API level — failures are
//! either represented as "not found" (`Option`, empty `Vec`) or surfaced
//! through the [`ExtensionValidationResult`] returned by
//! [`ExtensionLoader::validate`].

mod manifest;
mod result;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use systemprompt_models::DiscoveredExtension;

use manifest::{load_manifest, mtime_of};

pub use result::ExtensionValidationResult;

const CARGO_TARGET: &str = "target";

/// Stateless loader that walks the `extensions/` tree.
#[derive(Debug, Clone, Copy)]
pub struct ExtensionLoader;

impl ExtensionLoader {
    /// Discovers every extension manifest under `project_root/extensions/`.
    ///
    /// Manifests that fail to parse are logged at `warn` level and
    /// skipped — callers that need a strict result should compose this
    /// with [`Self::validate`].
    #[must_use]
    pub fn discover(project_root: &Path) -> Vec<DiscoveredExtension> {
        let extensions_dir = project_root.join("extensions");

        if !extensions_dir.exists() {
            return vec![];
        }

        let mut discovered = vec![];

        Self::scan_directory(&extensions_dir, &mut discovered);

        if let Ok(entries) = fs::read_dir(&extensions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::scan_directory(&path, &mut discovered);
                }
            }
        }

        discovered
    }

    fn scan_directory(dir: &Path, discovered: &mut Vec<DiscoveredExtension>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let ext_dir = entry.path();
            if !ext_dir.is_dir() {
                continue;
            }

            let manifest_path = ext_dir.join("manifest.yaml");
            if manifest_path.exists() {
                match load_manifest(&manifest_path) {
                    Ok(manifest) => {
                        discovered.push(DiscoveredExtension::new(manifest, ext_dir, manifest_path));
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
    }

    /// Returns every enabled MCP-kind extension under `project_root`.
    #[must_use]
    pub fn get_enabled_mcp_extensions(project_root: &Path) -> Vec<DiscoveredExtension> {
        Self::discover(project_root)
            .into_iter()
            .filter(|e| e.is_mcp() && e.is_enabled())
            .collect()
    }

    /// Returns every enabled CLI-kind extension under `project_root`.
    #[must_use]
    pub fn get_enabled_cli_extensions(project_root: &Path) -> Vec<DiscoveredExtension> {
        Self::discover(project_root)
            .into_iter()
            .filter(|e| e.is_cli() && e.is_enabled())
            .collect()
    }

    /// Locates a CLI extension by either binary name or manifest name.
    #[must_use]
    pub fn find_cli_extension(project_root: &Path, name: &str) -> Option<DiscoveredExtension> {
        Self::get_enabled_cli_extensions(project_root)
            .into_iter()
            .find(|e| {
                e.binary_name()
                    .is_some_and(|b| b == name || e.manifest.extension.name == name)
            })
    }

    /// Resolves the on-disk path of a CLI binary, preferring the release
    /// build over the debug build when both exist.
    #[must_use]
    pub fn get_cli_binary_path(
        project_root: &Path,
        binary_name: &str,
    ) -> Option<std::path::PathBuf> {
        let release_path = project_root
            .join(CARGO_TARGET)
            .join("release")
            .join(binary_name);
        if release_path.exists() {
            return Some(release_path);
        }

        let debug_path = project_root
            .join(CARGO_TARGET)
            .join("debug")
            .join(binary_name);
        if debug_path.exists() {
            return Some(debug_path);
        }

        None
    }

    /// Resolves the directory the runtime should run binaries from.
    ///
    /// `override_path` short-circuits the heuristic when the operator has
    /// pinned a directory; otherwise the function picks the more
    /// recently rebuilt of `target/release/systemprompt` vs
    /// `target/debug/systemprompt`, falling back to whichever exists.
    #[must_use]
    pub fn resolve_bin_directory(
        project_root: &Path,
        override_path: Option<&Path>,
    ) -> std::path::PathBuf {
        if let Some(path) = override_path {
            return path.to_path_buf();
        }

        let release_dir = project_root.join(CARGO_TARGET).join("release");
        let debug_dir = project_root.join(CARGO_TARGET).join("debug");

        let release_binary = release_dir.join("systemprompt");
        let debug_binary = debug_dir.join("systemprompt");

        match (release_binary.exists(), debug_binary.exists()) {
            (true, true) => {
                let release_mtime = mtime_of(&release_binary);
                let debug_mtime = mtime_of(&debug_binary);

                match (release_mtime, debug_mtime) {
                    (Some(r), Some(d)) if d > r => debug_dir,
                    _ => release_dir,
                }
            },
            (true | false, false) => release_dir,
            (false, true) => debug_dir,
        }
    }

    /// Returns the `(binary_name, manifest_dir)` pairs for every enabled
    /// MCP extension whose binary is missing from the resolved bin
    /// directory.
    #[must_use]
    pub fn validate_mcp_binaries(project_root: &Path) -> Vec<(String, std::path::PathBuf)> {
        let extensions = Self::get_enabled_mcp_extensions(project_root);
        let target_dir = Self::resolve_bin_directory(project_root, None);

        extensions
            .into_iter()
            .filter_map(|ext| {
                ext.binary_name().and_then(|binary| {
                    let binary_path = target_dir.join(binary);
                    if binary_path.exists() {
                        None
                    } else {
                        Some((binary.to_string(), ext.path.clone()))
                    }
                })
            })
            .collect()
    }

    /// Returns the names of every enabled MCP-extension binary, regardless
    /// of whether the binary currently exists on disk.
    #[must_use]
    pub fn get_mcp_binary_names(project_root: &Path) -> Vec<String> {
        Self::get_enabled_mcp_extensions(project_root)
            .iter()
            .filter_map(|e| e.binary_name().map(String::from))
            .collect()
    }

    /// Returns MCP-extension binary names filtered to the production
    /// subset (i.e. excludes deployments flagged `dev_only` in the
    /// services config).
    #[must_use]
    pub fn get_production_mcp_binary_names(
        project_root: &Path,
        services_config: &systemprompt_models::ServicesConfig,
    ) -> Vec<String> {
        Self::get_enabled_mcp_extensions(project_root)
            .iter()
            .filter_map(|e| {
                let binary = e.binary_name()?;
                let is_dev_only = services_config
                    .mcp_servers
                    .values()
                    .find(|d| d.binary == binary)
                    .is_some_and(|d| d.dev_only);
                (!is_dev_only).then(|| binary.to_string())
            })
            .collect()
    }

    /// Builds a binary-name → discovered-extension map for every manifest
    /// that exposes a binary.
    #[must_use]
    pub fn build_binary_map(project_root: &Path) -> HashMap<String, DiscoveredExtension> {
        Self::discover(project_root)
            .into_iter()
            .filter_map(|ext| {
                let name = ext.binary_name()?.to_string();
                Some((name, ext))
            })
            .collect()
    }

    /// Composite validation that returns every discovered extension and
    /// every missing binary in a single result struct.
    #[must_use]
    pub fn validate(project_root: &Path) -> ExtensionValidationResult {
        ExtensionValidationResult {
            discovered: Self::discover(project_root),
            missing_binaries: Self::validate_mcp_binaries(project_root),
            missing_manifests: vec![],
        }
    }
}
