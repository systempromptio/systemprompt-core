//! Base-directory lookups that honour the same overrides on every platform.
//!
//! `dirs` resolves per platform: `XDG_*` on Linux, `~/Library/Application
//! Support` on macOS, and the known-folder API on Windows, which consults
//! neither `HOME` nor `XDG_*`. So a caller that redirects those variables
//! redirects Linux alone and silently keeps reading the real user profile on
//! the other two — which is how the macOS and Windows bridge suites came to
//! assert against a live machine while appearing to run sandboxed.
//!
//! Every base-directory lookup in the bridge goes through this module, which
//! reads the override variable first and falls back to `dirs` when it is
//! absent. On Linux that first read is what `dirs` would have done anyway, so
//! behaviour there is unchanged; on macOS and Windows it becomes an override
//! that was previously impossible to express.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

fn env_dir(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    // Why: the XDG spec says to ignore empty or relative values; honouring one
    // would resolve writes against the process's cwd.
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

/// The override root alone, with no fallback.
///
/// The path ladders in [`crate::config::paths`] and [`crate::obs`] cannot use
/// the functions above: they hand-roll a different layout per platform —
/// `LOCALAPPDATA` on Windows, `Library/Application Support` on macOS — and only
/// the Linux arm ever consulted these variables. Each ladder consults its
/// override first and keeps its native layout as the fallback, so a caller that
/// sets one gets the same directory on all three platforms.
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
