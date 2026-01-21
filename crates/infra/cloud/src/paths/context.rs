#![allow(clippy::redundant_closure_for_method_calls)]

use std::path::{Path, PathBuf};

use super::{CloudPath, CloudPaths, DiscoveredProject};
use crate::constants::{dir_names, file_names};

#[derive(Debug, Clone)]
pub struct UnifiedContext {
    project: Option<DiscoveredProject>,
    cloud_paths: Option<CloudPaths>,
}

impl UnifiedContext {
    #[must_use]
    pub fn discover() -> Self {
        let project = DiscoveredProject::discover();
        Self {
            project,
            cloud_paths: None,
        }
    }

    #[must_use]
    pub fn discover_from(start: &Path) -> Self {
        let project = DiscoveredProject::discover_from(start);
        Self {
            project,
            cloud_paths: None,
        }
    }

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

    #[must_use]
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }

    #[must_use]
    pub fn project_root(&self) -> Option<&Path> {
        self.project.as_ref().map(|p| p.root())
    }

    #[must_use]
    pub fn systemprompt_dir(&self) -> Option<PathBuf> {
        self.project
            .as_ref()
            .map(DiscoveredProject::systemprompt_dir)
            .map(Path::to_path_buf)
    }

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

    #[must_use]
    pub fn profiles_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.profiles_dir())
    }

    #[must_use]
    pub fn profile_dir(&self, name: &str) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.profile_dir(name))
    }

    #[must_use]
    pub fn docker_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.docker_dir())
    }

    #[must_use]
    pub fn storage_dir(&self) -> Option<PathBuf> {
        self.project.as_ref().map(|p| p.storage_dir())
    }

    #[must_use]
    pub fn has_credentials(&self) -> bool {
        self.credentials_path().exists()
    }

    #[must_use]
    pub fn has_tenants(&self) -> bool {
        self.tenants_path().exists()
    }

    #[must_use]
    pub fn has_session(&self) -> bool {
        self.session_path().exists()
    }

    #[must_use]
    pub fn has_profile(&self, name: &str) -> bool {
        self.project
            .as_ref()
            .map(|p| p.has_profile(name))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn project(&self) -> Option<&DiscoveredProject> {
        self.project.as_ref()
    }

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
