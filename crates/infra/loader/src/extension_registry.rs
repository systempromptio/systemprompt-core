//! In-memory index of discovered extensions used to resolve binary paths
//! at runtime, with a separate code path for cloud deployments where
//! every binary lives in a single configured directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use systemprompt_models::DiscoveredExtension;

use crate::ExtensionLoader;
use crate::error::{ExtensionLoadError, ExtensionLoadResult};

#[derive(Debug)]
pub struct ExtensionRegistry {
    discovered: HashMap<String, DiscoveredExtension>,
    bin_path: PathBuf,
    is_cloud: bool,
}

impl ExtensionRegistry {
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

    #[must_use]
    pub fn get_extension(&self, binary_name: &str) -> Option<&DiscoveredExtension> {
        self.discovered.get(binary_name)
    }

    #[must_use]
    pub fn has_extension(&self, binary_name: &str) -> bool {
        self.bin_path.join(binary_name).exists() || self.discovered.contains_key(binary_name)
    }
}
