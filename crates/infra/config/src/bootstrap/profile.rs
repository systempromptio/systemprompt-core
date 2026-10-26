//! The profile an instance runs, loaded once and handed to its readers.
//!
//! [`LoadedProfile`] is the validated profile YAML together with the path it
//! was read from. The boot path builds it once; every layer that needs the
//! profile receives it explicitly rather than reading a process-wide cell.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_manifest::profile::{Profile, ProfileError};

use crate::error::ConfigResult;

#[derive(Debug)]
pub struct LoadedProfile {
    profile: Profile,
    path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProfileBootstrapError {
    #[error("Profile path not set. Set SYSTEMPROMPT_PROFILE environment variable")]
    PathNotSet,
}

impl LoadedProfile {
    pub fn from_env() -> ConfigResult<Arc<Self>> {
        let path_str = std::env::var("SYSTEMPROMPT_PROFILE")
            .map_err(|_e| ProfileBootstrapError::PathNotSet)?;
        Self::from_path(Path::new(&path_str))
    }

    pub fn from_path(path: &Path) -> ConfigResult<Arc<Self>> {
        let content = std::fs::read_to_string(path).map_err(|source| ProfileError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        let profile = Profile::from_yaml(&content, path)?;
        profile.validate()?;
        Ok(Arc::new(Self {
            profile,
            path: path.to_path_buf(),
        }))
    }

    #[must_use]
    pub const fn profile(&self) -> &Profile {
        &self.profile
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Deref for LoadedProfile {
    type Target = Profile;

    fn deref(&self) -> &Profile {
        &self.profile
    }
}
