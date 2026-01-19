//! Unified project context for path resolution.
//!
//! This module provides `UnifiedContext` which consolidates the functionality
//! of both `CloudPaths` (profile-based path resolution) and `ProjectContext`
//! (project structure discovery) into a single abstraction.

#![allow(clippy::redundant_closure_for_method_calls)]

use std::path::{Path, PathBuf};

use super::{CloudPath, CloudPaths, DiscoveredProject};
use crate::constants::{dir_names, file_names};

/// Unified context for path resolution.
///
/// Combines project discovery with cloud path resolution, providing a single
/// interface for all path-related operations.
#[derive(Debug, Clone)]
pub struct UnifiedContext {
    /// The discovered project, if any.
    project: Option<DiscoveredProject>,
    /// Cloud paths from profile configuration, if available.
    cloud_paths: Option<CloudPaths>,
}

impl UnifiedContext {
    /// Creates a new context by discovering the project from the current directory.
    #[must_use]
    pub fn discover() -> Self {
        let project = DiscoveredProject::discover();
        Self {
            project,
            cloud_paths: None,
        }
    }

    /// Creates a new context from a specific directory.
    #[must_use]
    pub fn discover_from(start: &Path) -> Self {
        let project = DiscoveredProject::discover_from(start);
        Self {
            project,
            cloud_paths: None,
        }
    }

    /// Creates a context with profile-based cloud path configuration.
    pub fn with_profile_paths(
        mut self,
        profile_dir: &Path,
        credentials_path: &str,
        tenants_path: &str,
    ) -> Self {
        self.cloud_paths = Some(CloudPaths::from_config(
            profile_dir,
            credentials_path,
            tenants_path,
        ));
        self
    }

    /// Returns whether a project was discovered.
    #[must_use]
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }

    /// Returns the project root, if discovered.
    #[must_use]
    pub fn project_root(&self) -> Option<&Path> {
        self.project.as_ref().map(|p| p.root())
    }

    /// Returns the `.systemprompt` directory path, if project was discovered.
    #[must_use]
    pub fn systemprompt_dir(&self) -> Option<PathBuf> {
        self.project
            .as_ref()
            .map(DiscoveredProject::systemprompt_dir)
            .map(Path::to_path_buf)
    }

    /// Resolves the credentials path.
    ///
    /// Priority:
    /// 1. Profile-configured cloud paths (if set)
    /// 2. Discovered project paths
    /// 3. Fallback to current directory
    #[must_use]
    pub fn credentials_path(&self) -> PathBuf {
        if let Some(cloud) = &self.cloud_paths {
            return cloud.resolve(CloudPath::Credentials);
        }
        if let Some(project) = &self.project {
            return project.credentials_path();
        }
        PathBuf::from(dir_names::SYSTEMPROMPT).join(file_names::CREDENTIALS)
    }

    /// Resolves the tenants path.
    #[must_use]
    pub fn tenants_path(&self) -> PathBuf {
        if let Some(cloud) = &self.cloud_paths {
            return cloud.resolve(CloudPath::Tenants);
        }
        if let Some(project) = &self.project {
            return project.tenants_path();
        }
        PathBuf::from(dir_names::SYSTEMPROMPT).join(file_names::TENANTS)
    }

    /// Resolves the session path.
    #[must_use]
    pub fn session_path(&self) -> PathBuf {
        if let Some(cloud) = &self.cloud_paths {
            return cloud.resolve(CloudPath::CliSession);
        }
        if let Some(project) = &self.project {
            return project.session_path();
        }
        PathBuf::from(dir_names::SYSTEMPROMPT).join(file_names::SESSION)
    }

    /// Resolves the profiles directory.
    #[must_use]
    pub fn profiles_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.profiles_dir())
    }

    /// Resolves a profile directory by name.
    #[must_use]
    pub fn profile_dir(&self, name: &str) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.profile_dir(name))
    }

    /// Resolves the docker directory.
    #[must_use]
    pub fn docker_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.docker_dir())
    }

    /// Resolves the storage directory.
    #[must_use]
    pub fn storage_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.storage_dir())
    }

    /// Checks if credentials exist.
    #[must_use]
    pub fn has_credentials(&self) -> bool {
        self.credentials_path().exists()
    }

    /// Checks if tenants exist.
    #[must_use]
    pub fn has_tenants(&self) -> bool {
        self.tenants_path().exists()
    }

    /// Checks if session exists.
    #[must_use]
    pub fn has_session(&self) -> bool {
        self.session_path().exists()
    }

    /// Checks if a profile exists.
    #[must_use]
    pub fn has_profile(&self, name: &str) -> bool {
        self.project
            .as_ref()
            .map(|p| p.has_profile(name))
            .unwrap_or(false)
    }

    /// Returns the underlying `DiscoveredProject`, if any.
    #[must_use]
    pub fn project(&self) -> Option<&DiscoveredProject> {
        self.project.as_ref()
    }

    /// Returns the underlying `CloudPaths`, if configured.
    #[must_use]
    pub fn cloud_paths(&self) -> Option<&CloudPaths> {
        self.cloud_paths.as_ref()
    }
}

impl Default for UnifiedContext {
    fn default() -> Self {
        Self::discover()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_with_project() {
        let temp = TempDir::new().unwrap();
        let systemprompt_dir = temp.path().join(".systemprompt");
        fs::create_dir(&systemprompt_dir).unwrap();

        let ctx = UnifiedContext::discover_from(temp.path());
        assert!(ctx.has_project());
        assert_eq!(ctx.project_root(), Some(temp.path()));
    }

    #[test]
    fn test_discover_without_project() {
        let temp = TempDir::new().unwrap();
        let ctx = UnifiedContext::discover_from(temp.path());
        assert!(!ctx.has_project());
    }

    #[test]
    fn test_path_resolution_with_project() {
        let temp = TempDir::new().unwrap();
        let systemprompt_dir = temp.path().join(".systemprompt");
        fs::create_dir(&systemprompt_dir).unwrap();

        let ctx = UnifiedContext::discover_from(temp.path());
        assert_eq!(
            ctx.credentials_path(),
            systemprompt_dir.join("credentials.json")
        );
        assert_eq!(ctx.tenants_path(), systemprompt_dir.join("tenants.json"));
        assert_eq!(ctx.session_path(), systemprompt_dir.join("session.json"));
    }
}
