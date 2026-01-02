use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use systemprompt_models::DiscoveredExtension;

use crate::ExtensionLoader;

#[derive(Debug)]
pub struct ExtensionRegistry {
    discovered: HashMap<String, DiscoveredExtension>,
    mcp_path: Option<PathBuf>,
}

impl ExtensionRegistry {
    pub fn build(project_root: &Path) -> Self {
        let mcp_path = std::env::var("SYSTEMPROMPT_MCP_PATH")
            .ok()
            .map(PathBuf::from);

        Self {
            discovered: ExtensionLoader::build_binary_map(project_root),
            mcp_path,
        }
    }

    pub fn get_path(&self, binary_name: &str) -> Result<PathBuf> {
        if let Some(ref mcp_path) = self.mcp_path {
            let binary_path = mcp_path.join(binary_name);
            if binary_path.exists() {
                return Ok(mcp_path.clone());
            }
        }

        self.discovered
            .get(binary_name)
            .map(|ext| ext.path.clone())
            .ok_or_else(|| anyhow::anyhow!("No manifest.yaml found for binary '{}'", binary_name))
    }

    pub fn get_extension(&self, binary_name: &str) -> Option<&DiscoveredExtension> {
        self.discovered.get(binary_name)
    }

    pub fn has_extension(&self, binary_name: &str) -> bool {
        self.mcp_path
            .as_ref()
            .is_some_and(|p| p.join(binary_name).exists())
            || self.discovered.contains_key(binary_name)
    }
}
