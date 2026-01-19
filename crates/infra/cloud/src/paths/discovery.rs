use std::path::{Path, PathBuf};

use crate::constants::{dir_names, file_names};

#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    root: PathBuf,
    systemprompt_dir: PathBuf,
}

impl DiscoveredProject {
    #[must_use]
    pub fn discover() -> Option<Self> {
        let cwd = std::env::current_dir().ok()?;
        Self::discover_from(&cwd)
    }

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

    #[must_use]
    pub fn from_root(root: PathBuf) -> Self {
        let systemprompt_dir = root.join(dir_names::SYSTEMPROMPT);
        Self {
            root,
            systemprompt_dir,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn systemprompt_dir(&self) -> &Path {
        &self.systemprompt_dir
    }

    #[must_use]
    pub fn credentials_path(&self) -> PathBuf {
        self.systemprompt_dir.join(file_names::CREDENTIALS)
    }

    #[must_use]
    pub fn tenants_path(&self) -> PathBuf {
        self.systemprompt_dir.join(file_names::TENANTS)
    }

    #[must_use]
    pub fn session_path(&self) -> PathBuf {
        self.systemprompt_dir.join(file_names::SESSION)
    }

    #[must_use]
    pub fn sessions_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::SESSIONS)
    }

    #[must_use]
    pub fn profiles_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::PROFILES)
    }

    #[must_use]
    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(name)
    }

    #[must_use]
    pub fn profile_config(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join(file_names::PROFILE_CONFIG)
    }

    #[must_use]
    pub fn profile_secrets(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join(file_names::PROFILE_SECRETS)
    }

    #[must_use]
    pub fn docker_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::DOCKER)
    }

    #[must_use]
    pub fn storage_dir(&self) -> PathBuf {
        self.systemprompt_dir.join(dir_names::STORAGE)
    }

    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.systemprompt_dir.is_dir()
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
        self.profile_dir(name).is_dir()
    }
}
