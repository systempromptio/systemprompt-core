//! Process-wide profile bootstrap.
//!
//! Loads the active profile YAML from `SYSTEMPROMPT_PROFILE` (or an
//! explicit path) and stores it in a `OnceLock` so the rest of the
//! application can access it without passing it down call stacks.

use std::path::Path;
use std::sync::OnceLock;

use systemprompt_models::profile::Profile;

use crate::error::ConfigResult;

static PROFILE: OnceLock<Profile> = OnceLock::new();
static PROFILE_PATH: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct ProfileBootstrap;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProfileBootstrapError {
    #[error("Profile not initialized. Call ProfileBootstrap::init() at application startup")]
    NotInitialized,

    #[error("Profile already initialized")]
    AlreadyInitialized,

    #[error("Profile path not set. Set SYSTEMPROMPT_PROFILE environment variable")]
    PathNotSet,

    #[error("Profile validation failed: {0}")]
    ValidationFailed(String),

    #[error("Failed to load profile: {0}")]
    LoadFailed(String),
}

impl ProfileBootstrap {
    pub fn init() -> ConfigResult<&'static Profile> {
        if PROFILE.get().is_some() {
            return Err(ProfileBootstrapError::AlreadyInitialized.into());
        }

        let path_str =
            std::env::var("SYSTEMPROMPT_PROFILE").map_err(|_| ProfileBootstrapError::PathNotSet)?;
        let path = std::path::PathBuf::from(path_str);

        let profile = Self::load_from_path_and_validate(&path)?;

        PROFILE_PATH
            .set(path.to_string_lossy().to_string())
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .set(profile)
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .get()
            .ok_or_else(|| ProfileBootstrapError::NotInitialized.into())
    }

    pub fn get() -> Result<&'static Profile, ProfileBootstrapError> {
        PROFILE.get().ok_or(ProfileBootstrapError::NotInitialized)
    }

    pub fn get_path() -> Result<&'static str, ProfileBootstrapError> {
        PROFILE_PATH
            .get()
            .map(String::as_str)
            .ok_or(ProfileBootstrapError::NotInitialized)
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        PROFILE.get().is_some()
    }

    pub fn try_init() -> ConfigResult<&'static Profile> {
        if let Some(profile) = PROFILE.get() {
            return Ok(profile);
        }
        Self::init()
    }

    pub fn init_from_path(path: &Path) -> ConfigResult<&'static Profile> {
        if PROFILE.get().is_some() {
            return Err(ProfileBootstrapError::AlreadyInitialized.into());
        }

        let profile = Self::load_from_path_and_validate(path)?;

        PROFILE_PATH
            .set(path.to_string_lossy().to_string())
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .set(profile)
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .get()
            .ok_or_else(|| ProfileBootstrapError::NotInitialized.into())
    }

    fn load_from_path_and_validate(path: &Path) -> ConfigResult<Profile> {
        let profile = crate::profile_loader::load_profile_with_catalog(path)?;
        profile.validate()?;
        Ok(profile)
    }
}
