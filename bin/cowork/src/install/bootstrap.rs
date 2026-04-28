use crate::paths::{self, OrgPluginsLocation};
use std::fs;
use std::path::Path;

pub fn bootstrap_directory(loc: &OrgPluginsLocation) -> std::io::Result<()> {
    fs::create_dir_all(&loc.path)?;
    let meta = paths::metadata_dir(&loc.path);
    fs::create_dir_all(&meta)?;
    Ok(())
}

pub fn write_version_sentinel(
    org_plugins: &Path,
    binary: &Path,
    gateway_url: Option<&str>,
) -> std::io::Result<()> {
    let sentinel = paths::metadata_dir(org_plugins).join(paths::VERSION_SENTINEL);
    let payload = serde_json::json!({
        "binary": binary.display().to_string(),
        "binary_version": env!("CARGO_PKG_VERSION"),
        "installed_at": current_iso8601(),
        "gateway_url": gateway_url,
    });
    fs::write(
        &sentinel,
        serde_json::to_vec_pretty(&payload).unwrap_or_default(),
    )?;
    Ok(())
}

pub fn current_iso8601() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}
