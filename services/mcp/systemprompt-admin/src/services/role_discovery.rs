use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use systemprompt::models::extension::{ExtensionManifest, ManifestRole};

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredRole {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub source: RoleSource,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum RoleSource {
    Core,
    Extension { extension_name: String },
}

impl DiscoveredRole {
    pub fn core(name: &str, display_name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            source: RoleSource::Core,
            permissions: Vec::new(),
        }
    }

    pub fn from_manifest(role_name: &str, role: &ManifestRole, extension_name: &str) -> Self {
        Self {
            name: role_name.to_string(),
            display_name: role.display_name.clone(),
            description: role.description.clone(),
            source: RoleSource::Extension {
                extension_name: extension_name.to_string(),
            },
            permissions: role.permissions.clone(),
        }
    }
}

pub struct RoleDiscoveryService {
    extensions_path: std::path::PathBuf,
}

impl RoleDiscoveryService {
    pub fn new(extensions_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            extensions_path: extensions_path.into(),
        }
    }

    pub async fn discover_all_roles(&self) -> Result<Vec<DiscoveredRole>> {
        let mut roles = self.core_roles();

        let extension_roles = self.discover_extension_roles().await?;
        roles.extend(extension_roles);

        Ok(roles)
    }

    pub fn core_roles(&self) -> Vec<DiscoveredRole> {
        vec![
            DiscoveredRole::core("anonymous", "Anonymous", "Unauthenticated user"),
            DiscoveredRole::core("user", "User", "Authenticated user"),
            DiscoveredRole::core("admin", "Admin", "Full system administrator"),
        ]
    }

    pub fn role_names(&self, roles: &[DiscoveredRole]) -> Vec<String> {
        roles.iter().map(|r| r.name.clone()).collect()
    }

    async fn discover_extension_roles(&self) -> Result<Vec<DiscoveredRole>> {
        let mut roles = Vec::new();

        let entries = match std::fs::read_dir(&self.extensions_path) {
            Ok(entries) => entries,
            Err(_) => return Ok(roles),
        };

        for entry in entries.flatten() {
            let manifest_path = entry.path().join("manifest.yaml");
            if manifest_path.exists() {
                if let Ok(manifest) = self.load_manifest(&manifest_path).await {
                    for (role_name, role_def) in &manifest.extension.roles {
                        roles.push(DiscoveredRole::from_manifest(
                            role_name,
                            role_def,
                            &manifest.extension.name,
                        ));
                    }
                }
            }
        }

        Ok(roles)
    }

    async fn load_manifest(&self, path: &Path) -> Result<ExtensionManifest> {
        let content = tokio::fs::read_to_string(path).await?;
        let manifest: ExtensionManifest = serde_yaml::from_str(&content)?;
        Ok(manifest)
    }
}

pub fn default_core_roles() -> Vec<DiscoveredRole> {
    vec![
        DiscoveredRole::core("anonymous", "Anonymous", "Unauthenticated user"),
        DiscoveredRole::core("user", "User", "Authenticated user"),
        DiscoveredRole::core("admin", "Admin", "Full system administrator"),
    ]
}
