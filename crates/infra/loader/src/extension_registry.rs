//! In-memory index of discovered extensions used to resolve binary paths
//! at runtime, with a separate code path for cloud deployments where
//! every binary lives in a single configured directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use systemprompt_models::DiscoveredExtension;

use crate::ExtensionLoader;
use crate::error::{ExtensionLoadError, ExtensionLoadResult};

/// Resolves extension binaries from the appropriate lookup table for the
/// current deployment mode (cloud vs. self-hosted).
#[derive(Debug)]
pub struct ExtensionRegistry {
    discovered: HashMap<String, DiscoveredExtension>,
    bin_path: PathBuf,
    is_cloud: bool,
}

impl ExtensionRegistry {
    /// Builds the registry for `project_root`.
    ///
    /// In `is_cloud` mode the registry is empty and binaries are looked
    /// up directly in `bin_path`; in self-hosted mode the registry is
    /// populated by [`ExtensionLoader::build_binary_map`].
    #[must_use]
    pub fn build(project_root: &Path, is_cloud: bool, bin_path: &str) -> Self {
        let discovered = if is_cloud {
            HashMap::new()
        } else {
            ExtensionLoader::build_binary_map(project_root)
        };

        Self {
            discovered,
            bin_path: PathBuf::from(bin_path),
            is_cloud,
        }
    }

    /// Returns the directory containing the binary named `binary_name`.
    ///
    /// # Errors
    ///
    /// Returns [`ExtensionLoadError::BinaryNotFound`] in cloud mode if
    /// `bin_path/binary_name` does not exist, or
    /// [`ExtensionLoadError::ManifestMissing`] in self-hosted mode if no
    /// manifest declares `binary_name`.
    pub fn get_path(&self, binary_name: &str) -> ExtensionLoadResult<PathBuf> {
        if self.is_cloud {
            let binary_path = self.bin_path.join(binary_name);
            return binary_path
                .exists()
                .then(|| self.bin_path.clone())
                .ok_or_else(|| ExtensionLoadError::BinaryNotFound {
                    name: binary_name.to_string(),
                    path: binary_path,
                });
        }

        self.discovered
            .get(binary_name)
            .map(|ext| ext.path.clone())
            .ok_or_else(|| ExtensionLoadError::ManifestMissing(binary_name.to_string()))
    }

    /// Returns the discovered extension for `binary_name`, if any.
    #[must_use]
    pub fn get_extension(&self, binary_name: &str) -> Option<&DiscoveredExtension> {
        self.discovered.get(binary_name)
    }

    /// Returns `true` if a usable binary exists for `binary_name` in
    /// either lookup table.
    #[must_use]
    pub fn has_extension(&self, binary_name: &str) -> bool {
        self.bin_path.join(binary_name).exists() || self.discovered.contains_key(binary_name)
    }
}
