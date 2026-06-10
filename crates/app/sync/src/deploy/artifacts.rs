//! Pre-deploy validation of the project's build artifacts.
//!
//! [`DeployArtifacts`] locates the release binary and profile Dockerfile,
//! then asserts that required extension assets, storage, and template
//! directories exist inside the Docker build context before a cloud deploy
//! proceeds.

use std::path::{Path, PathBuf};

use systemprompt_cloud::ProjectContext;
use systemprompt_extension::{AssetPaths, ExtensionRegistry};
use systemprompt_models::paths::constants::build;

use crate::error::{SyncError, SyncResult};

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
pub struct DeployArtifacts {
    pub binary: PathBuf,
    pub dockerfile: PathBuf,
    project_root: PathBuf,
}

impl DeployArtifacts {
    pub fn resolve(project_root: &Path, profile_name: &str) -> SyncResult<Self> {
        let binary = project_root
            .join(build::CARGO_TARGET)
            .join("release")
            .join(build::BINARY_NAME);

        let ctx = ProjectContext::new(project_root.to_path_buf());
        let dockerfile = ctx.profile_dockerfile(profile_name);

        let artifacts = Self {
            binary,
            dockerfile,
            project_root: project_root.to_path_buf(),
        };
        artifacts.validate()?;
        Ok(artifacts)
    }

    fn validate(&self) -> SyncResult<()> {
        if !self.binary.exists() {
            return Err(SyncError::BuildArtifacts(format!(
                "Release binary not found: {}\n\nRun: cargo build --release --bin systemprompt",
                self.binary.display()
            )));
        }

        self.validate_extension_assets()?;
        self.validate_storage_directory()?;
        self.validate_templates_directory()?;

        if !self.dockerfile.exists() {
            return Err(SyncError::BuildArtifacts(format!(
                "Dockerfile not found: {}\n\nCreate a Dockerfile at this location",
                self.dockerfile.display()
            )));
        }

        Ok(())
    }

    fn validate_extension_assets(&self) -> SyncResult<()> {
        let paths = ProjectAssetPaths {
            storage_files: self.project_root.join("storage/files"),
            web_dist: self.project_root.join("web/dist"),
        };
        let registry = ExtensionRegistry::discover()?;
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
            return Err(SyncError::BuildArtifacts(format!(
                "Missing required extension assets:\n  {}\n\nCreate these files or mark them as \
                 optional.",
                missing.join("\n  ")
            )));
        }

        if !outside_context.is_empty() {
            return Err(SyncError::BuildArtifacts(format!(
                "Extension assets outside Docker build context:\n  {}\n\nMove assets inside the \
                 project directory.",
                outside_context.join("\n  ")
            )));
        }

        Ok(())
    }

    fn validate_storage_directory(&self) -> SyncResult<()> {
        let storage_dir = self.project_root.join("storage");

        if !storage_dir.exists() {
            return Err(SyncError::BuildArtifacts(format!(
                "Storage directory not found: {}\n\nExpected: storage/\n\nCreate this directory \
                 for files, images, and other assets.",
                storage_dir.display()
            )));
        }

        let files_dir = storage_dir.join("files");
        if !files_dir.exists() {
            return Err(SyncError::BuildArtifacts(format!(
                "Storage files directory not found: {}\n\nExpected: storage/files/\n\nThis \
                 directory is required for serving static assets.",
                files_dir.display()
            )));
        }

        Ok(())
    }

    fn validate_templates_directory(&self) -> SyncResult<()> {
        let templates_dir = self.project_root.join("services/web/templates");

        if !templates_dir.exists() {
            return Err(SyncError::BuildArtifacts(format!(
                "Templates directory not found: {}\n\nExpected: \
                 services/web/templates/\n\nCreate this directory with your HTML templates.",
                templates_dir.display()
            )));
        }

        Ok(())
    }
}
