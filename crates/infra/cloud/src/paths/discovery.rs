//! Unified project discovery.
//!
//! This module provides a clean interface for discovering project roots
//! and resolving cloud-related paths within a project structure.

use std::path::{Path, PathBuf};

use crate::constants::{dir_names, file_names};

/// Result of discovering a `SystemPrompt` project.
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    /// The root directory of the project (parent of .systemprompt).
    root: PathBuf,
    /// The .systemprompt directory within the project.
    systemprompt_dir: PathBuf,
}

impl DiscoveredProject {
    /// Attempts to discover a project starting from the current directory.
    ///
    /// Walks up the directory tree looking for a `.systemprompt` directory.
    /// Returns `Some` if found, `None` otherwise.
    #[must_use]
    pub fn discover() -> Option<Self> {
        let cwd = std::env::current_dir().ok()?;
        Self::discover_from(&cwd)
    }

    /// Attempts to discover a project starting from a given path.
    #[must_use]
    pub fn discover_from(start: &Path) -> Option<Self> {
        let mut current = start.to_path_buf();
        loop {
            let systemprompt_dir = current.join(dir_names::SYSTEMPROMPT);
            if systemprompt_dir.is_dir() {
                return Some(Self {
                    root: current,
                    systemprompt_dir,
                });
            }
            if !current.pop() {
                break;
            }
        }
        None
    }

    /// Creates a `DiscoveredProject` from a known root directory.
    ///
    /// Does not validate that the .systemprompt directory exists.
    #[must_use]
    pub fn from_root(root: PathBuf) -> Self {
        let systemprompt_dir = root.join(dir_names::SYSTEMPROMPT);
        Self {
            root,
            systemprompt_dir,
        }
    }

    /// Returns the project root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the .systemprompt directory path.
    #[must_use]
    pub fn systemprompt_dir(&self) -> &Path {
        &self.systemprompt_dir
    }

    /// Returns the path to the credentials file.
    #[must_use]
    pub fn credentials_path(&self) -> PathBuf {
        self.systemprompt_dir.join(file_names::CREDENTIALS)
    }

    /// Returns the path to the tenants file.
    #[must_use]
    pub fn tenants_path(&self) -> PathBuf {
        self.systemprompt_dir.join(file_names::TENANTS)
    }

    /// Returns the path to the session file.
    #[must_use]
    pub fn session_path(&self) -> PathBuf {
        self.systemprompt_dir.join(file_names::SESSION)
    }

    /// Returns the path to the profiles directory.
    #[must_use]
    pub fn profiles_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::PROFILES)
    }

    /// Returns the path to a specific profile directory.
    #[must_use]
    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(name)
    }

    /// Returns the path to a profile's config file.
    #[must_use]
    pub fn profile_config(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join(file_names::PROFILE_CONFIG)
    }

    /// Returns the path to a profile's secrets file.
    #[must_use]
    pub fn profile_secrets(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join(file_names::PROFILE_SECRETS)
    }

    /// Returns the path to the docker directory.
    #[must_use]
    pub fn docker_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::DOCKER)
    }

    /// Returns the path to the storage directory.
    #[must_use]
    pub fn storage_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::STORAGE)
    }

    /// Checks if this project has been initialized (has .systemprompt directory).
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.systemprompt_dir.is_dir()
    }

    /// Checks if credentials exist.
    #[must_use]
    pub fn has_credentials(&self) -> bool {
        self.credentials_path().exists()
    }

    /// Checks if tenants configuration exists.
    #[must_use]
    pub fn has_tenants(&self) -> bool {
        self.tenants_path().exists()
    }

    /// Checks if a session file exists.
    #[must_use]
    pub fn has_session(&self) -> bool {
        self.session_path().exists()
    }

    /// Checks if a specific profile exists.
    #[must_use]
    pub fn has_profile(&self, name: &str) -> bool {
        self.profile_dir(name).is_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_from_project_root() {
        let temp = TempDir::new().unwrap();
        let systemprompt_dir = temp.path().join(".systemprompt");
        fs::create_dir(&systemprompt_dir).unwrap();

        let project = DiscoveredProject::discover_from(temp.path()).unwrap();
        assert_eq!(project.root(), temp.path());
        assert_eq!(project.systemprompt_dir(), systemprompt_dir);
    }

    #[test]
    fn test_discover_from_subdirectory() {
        let temp = TempDir::new().unwrap();
        let systemprompt_dir = temp.path().join(".systemprompt");
        fs::create_dir(&systemprompt_dir).unwrap();

        let subdir = temp.path().join("src").join("nested");
        fs::create_dir_all(&subdir).unwrap();

        let project = DiscoveredProject::discover_from(&subdir).unwrap();
        assert_eq!(project.root(), temp.path());
    }

    #[test]
    fn test_discover_no_project() {
        let temp = TempDir::new().unwrap();
        let project = DiscoveredProject::discover_from(temp.path());
        assert!(project.is_none());
    }

    #[test]
    fn test_path_methods() {
        let temp = TempDir::new().unwrap();
        let project = DiscoveredProject::from_root(temp.path().to_path_buf());

        assert_eq!(
            project.credentials_path(),
            temp.path().join(".systemprompt/credentials.json")
        );
        assert_eq!(
            project.tenants_path(),
            temp.path().join(".systemprompt/tenants.json")
        );
        assert_eq!(
            project.session_path(),
            temp.path().join(".systemprompt/session.json")
        );
    }
}
