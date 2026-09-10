//! Base-directory resolution with explicit environment overrides.
//!
//! Checks the configured override before platform-specific `dirs` defaults.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

pub(crate) fn env_dir(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    // Why: XDG requires empty and relative directory overrides to be ignored.
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    env_dir("HOME").or_else(dirs::home_dir)
}

#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    env_dir("XDG_CONFIG_HOME").or_else(dirs::config_dir)
}

#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    env_dir("XDG_CACHE_HOME").or_else(dirs::cache_dir)
}

#[must_use]
pub fn data_local_dir() -> Option<PathBuf> {
    env_dir("XDG_DATA_HOME").or_else(dirs::data_local_dir)
}

#[must_use]
pub fn desktop_dir() -> Option<PathBuf> {
    dirs::desktop_dir()
}

#[must_use]
pub fn config_home_override() -> Option<PathBuf> {
    env_dir("XDG_CONFIG_HOME")
}

#[must_use]
pub fn data_home_override() -> Option<PathBuf> {
    env_dir("XDG_DATA_HOME")
}

#[must_use]
pub fn state_home_override() -> Option<PathBuf> {
    env_dir("XDG_STATE_HOME")
}
