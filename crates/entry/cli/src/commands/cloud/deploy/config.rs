use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use systemprompt_cloud::constants::build;
use systemprompt_cloud::ProjectContext;
use systemprompt_extension::{AssetPaths, ExtensionRegistry};

use crate::shared::project::ProjectRoot;

struct ProjectAssetPaths {
    storage_files: PathBuf,
    web_dist: PathBuf,
}

impl AssetPaths for ProjectAssetPaths {
    fn storage_files(&self) -> &Path {
        &self.storage_files
    }
    fn web_dist(&self) -> &Path {
        &self.web_dist
    }
}

#[derive(Debug)]
pub struct DeployConfig {
    pub binary: PathBuf,
    pub dockerfile: PathBuf,
    project_root: PathBuf,
}

impl DeployConfig {
    pub fn from_project(project: &ProjectRoot, profile_name: &str) -> Result<Self> {
        let root = project.as_path();
        let binary = root
            .join(build::CARGO_TARGET)
            .join("release")
            .join(build::BINARY_NAME);

        let ctx = ProjectContext::new(root.to_path_buf());
        let dockerfile = ctx.profile_dockerfile(profile_name);

        let config = Self {
            binary,
            dockerfile,
            project_root: root.to_path_buf(),
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if !self.binary.exists() {
            return Err(anyhow!(
                "Release binary not found: {}\n\nRun: cargo build --release --bin systemprompt",
                self.binary.display()
            ));
        }

        self.validate_extension_assets()?;
        self.validate_storage_directory()?;
        self.validate_templates_directory()?;

        if !self.dockerfile.exists() {
            return Err(anyhow!(
                "Dockerfile not found: {}\n\nCreate a Dockerfile at this location",
                self.dockerfile.display()
            ));
        }

        Ok(())
    }

    fn validate_extension_assets(&self) -> Result<()> {
        let paths = ProjectAssetPaths {
            storage_files: self.project_root.join("storage/files"),
            web_dist: self.project_root.join("web/dist"),
        };
        let registry = ExtensionRegistry::discover();
        let mut missing = Vec::new();
        let mut outside_context = Vec::new();

        for ext in registry.asset_extensions() {
            let ext_id = ext.id();
            for asset in ext.required_assets(&paths) {
                if !asset.is_required() {
                    continue;
                }

                let source = asset.source();

                if !source.exists() {
                    missing.push(format!("[ext:{}] {}", ext_id, source.display()));
                    continue;
                }

                if !source.starts_with(&self.project_root) {
                    outside_context.push(format!(
                        "[ext:{}] {} (not under {})",
                        ext_id,
                        source.display(),
                        self.project_root.display()
                    ));
                }
            }
        }

        if !missing.is_empty() {
            bail!(
                "Missing required extension assets:\n  {}\n\nCreate these files or mark them as \
                 optional.",
                missing.join("\n  ")
            );
        }

        if !outside_context.is_empty() {
            bail!(
                "Extension assets outside Docker build context:\n  {}\n\nMove assets inside the \
                 project directory.",
                outside_context.join("\n  ")
            );
        }

        Ok(())
    }

    fn validate_storage_directory(&self) -> Result<()> {
        let storage_dir = self.project_root.join("storage");

        if !storage_dir.exists() {
            bail!(
                "Storage directory not found: {}\n\nExpected: storage/\n\nCreate this directory \
                 for files, images, and other assets.",
                storage_dir.display()
            );
        }

        let files_dir = storage_dir.join("files");
        if !files_dir.exists() {
            bail!(
                "Storage files directory not found: {}\n\nExpected: storage/files/\n\nThis \
                 directory is required for serving static assets.",
                files_dir.display()
            );
        }

        Ok(())
    }

    fn validate_templates_directory(&self) -> Result<()> {
        let templates_dir = self.project_root.join("services/web/templates");

        if !templates_dir.exists() {
            bail!(
                "Templates directory not found: {}\n\nExpected: services/web/templates/\n\nCreate \
                 this directory with your HTML templates.",
                templates_dir.display()
            );
        }

        Ok(())
    }
}
