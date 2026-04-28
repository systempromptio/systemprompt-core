use crate::paths::{self, OrgPluginsLocation};
use std::fs;
use std::path::Path;

pub fn bootstrap_directory(loc: &OrgPluginsLocation) -> std::io::Result<()> {
    fs::create_dir_all(&loc.path)?;
    let meta = paths::metadata_dir(&loc.path);
    fs::create_dir_all(&meta)?;
    chown_to_sudo_user_if_root(&loc.path);
    chown_to_sudo_user_if_root(&meta);
    Ok(())
}

#[cfg(unix)]
fn chown_to_sudo_user_if_root(path: &Path) {
    use std::os::unix::fs::MetadataExt;
    let Ok(sudo_user) = std::env::var("SUDO_USER") else {
        return;
    };
    if sudo_user.is_empty() || sudo_user == "root" {
        return;
    }
    let Some((uid, gid)) = lookup_uid_gid(&sudo_user) else {
        tracing::warn!(user = %sudo_user, "could not resolve SUDO_USER; leaving ownership as root");
        return;
    };
    let needs_chown = std::fs::metadata(path)
        .map(|m| m.uid() != uid || m.gid() != gid)
        .unwrap_or(true);
    if !needs_chown {
        return;
    }
    let status = std::process::Command::new("/usr/sbin/chown")
        .arg("-R")
        .arg(format!("{uid}:{gid}"))
        .arg(path)
        .status();
    match status {
        Ok(s) if s.success() => {
            tracing::info!(path = %path.display(), user = %sudo_user, "chowned org-plugins to invoking user");
        },
        Ok(s) => tracing::warn!(path = %path.display(), exit = ?s.code(), "chown returned non-zero"),
        Err(e) => tracing::warn!(path = %path.display(), error = %e, "chown failed to spawn"),
    }
}

#[cfg(not(unix))]
fn chown_to_sudo_user_if_root(_path: &Path) {}

#[cfg(unix)]
fn lookup_uid_gid(user: &str) -> Option<(u32, u32)> {
    let output = std::process::Command::new("/usr/bin/id")
        .arg("-u")
        .arg(user)
        .output()
        .ok()?;
    let uid: u32 = std::str::from_utf8(&output.stdout).ok()?.trim().parse().ok()?;
    let output = std::process::Command::new("/usr/bin/id")
        .arg("-g")
        .arg(user)
        .output()
        .ok()?;
    let gid: u32 = std::str::from_utf8(&output.stdout).ok()?.trim().parse().ok()?;
    Some((uid, gid))
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
