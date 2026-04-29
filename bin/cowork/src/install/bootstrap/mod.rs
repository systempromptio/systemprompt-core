#[cfg(unix)]
mod unix;
#[cfg(not(unix))]
mod windows;

#[cfg(unix)]
use unix as os;
#[cfg(not(unix))]
use windows as os;

use crate::config::paths::{self, OrgPluginsLocation};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct VersionSentinel<'a> {
    binary: String,
    binary_version: &'a str,
    installed_at: String,
    gateway_url: Option<&'a str>,
}

pub fn bootstrap_directory(loc: &OrgPluginsLocation) -> std::io::Result<()> {
    fs::create_dir_all(&loc.path)?;
    let meta = paths::metadata_dir(&loc.path);
    fs::create_dir_all(&meta)?;
    os::chown_to_sudo_user_if_root(&loc.path);
    os::chown_to_sudo_user_if_root(&meta);
    Ok(())
}

pub fn write_version_sentinel(
    org_plugins: &Path,
    binary: &Path,
    gateway_url: Option<&str>,
) -> std::io::Result<()> {
    let sentinel = paths::metadata_dir(org_plugins).join(paths::VERSION_SENTINEL);
    let payload = VersionSentinel {
        binary: binary.display().to_string(),
        binary_version: env!("CARGO_PKG_VERSION"),
        installed_at: current_iso8601(),
        gateway_url,
    };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(std::io::Error::other)?;
    fs::write(&sentinel, bytes)?;
    Ok(())
}

pub fn current_iso8601() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}
