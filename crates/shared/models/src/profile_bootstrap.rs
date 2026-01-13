//! Profile bootstrap system for SystemPrompt applications.
//!
//! This module provides a global profile initialization system that ensures
//! profiles are the SINGLE source of truth for all configuration.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::OnceLock;

use crate::profile::Profile;

static PROFILE: OnceLock<Profile> = OnceLock::new();
static PROFILE_PATH: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct ProfileBootstrap;

#[derive(Debug, thiserror::Error)]
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
    /// Initialize the application with a profile.
    /// This MUST be called at startup before any config access.
    ///
    /// Reads profile path from SYSTEMPROMPT_PROFILE environment variable.
    /// The env var must contain the full path to the profile file.
    ///
    /// Returns error if profile cannot be loaded or validated.
    pub fn init() -> Result<&'static Profile> {
        if PROFILE.get().is_some() {
            anyhow::bail!(ProfileBootstrapError::AlreadyInitialized);
        }

        let path_str =
            std::env::var("SYSTEMPROMPT_PROFILE").map_err(|_| ProfileBootstrapError::PathNotSet)?;
        let path = std::path::PathBuf::from(path_str);

        let profile = Self::load_from_path_and_validate(&path)
            .with_context(|| format!("Failed to initialize profile from: {}", path.display()))?;

        PROFILE_PATH
            .set(path.to_string_lossy().to_string())
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .set(profile)
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .get()
            .ok_or_else(|| anyhow::anyhow!(ProfileBootstrapError::NotInitialized))
    }

    /// Get the initialized profile.
    /// Returns error if profile has not been initialized.
    pub fn get() -> Result<&'static Profile, ProfileBootstrapError> {
        PROFILE.get().ok_or(ProfileBootstrapError::NotInitialized)
    }

    /// Get the initialized profile path.
    /// Returns error if profile has not been initialized.
    pub fn get_path() -> Result<&'static str, ProfileBootstrapError> {
        PROFILE_PATH
            .get()
            .map(String::as_str)
            .ok_or(ProfileBootstrapError::NotInitialized)
    }

    /// Check if the profile has been initialized.
    pub fn is_initialized() -> bool {
        PROFILE.get().is_some()
    }

    pub fn try_init() -> Result<&'static Profile> {
        if let Some(profile) = PROFILE.get() {
            return Ok(profile);
        }
        Self::init()
    }

    pub fn init_from_path(path: &Path) -> Result<&'static Profile> {
        if PROFILE.get().is_some() {
            anyhow::bail!(ProfileBootstrapError::AlreadyInitialized);
        }

        let profile = Self::load_from_path_and_validate(path)
            .with_context(|| format!("Failed to initialize profile from: {}", path.display()))?;

        PROFILE_PATH
            .set(path.to_string_lossy().to_string())
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .set(profile)
            .map_err(|_| ProfileBootstrapError::AlreadyInitialized)?;

        PROFILE
            .get()
            .ok_or_else(|| anyhow::anyhow!(ProfileBootstrapError::NotInitialized))
    }

    fn load_from_path_and_validate(path: &Path) -> Result<Profile> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read profile: {}", path.display()))?;

        let profile = Profile::parse(&content, path)?;
        profile.validate()?;
        Ok(profile)
    }
}
