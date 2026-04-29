#![cfg(unix)]

use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub(super) fn chown_to_sudo_user_if_root(path: &Path) {
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
        Ok(s) => {
            tracing::warn!(path = %path.display(), exit = ?s.code(), "chown returned non-zero")
        },
        Err(e) => tracing::warn!(path = %path.display(), error = %e, "chown failed to spawn"),
    }
}

fn lookup_uid_gid(user: &str) -> Option<(u32, u32)> {
    let output = std::process::Command::new("/usr/bin/id")
        .arg("-u")
        .arg(user)
        .output()
        .ok()?;
    let uid: u32 = std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let output = std::process::Command::new("/usr/bin/id")
        .arg("-g")
        .arg(user)
        .output()
        .ok()?;
    let gid: u32 = std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some((uid, gid))
}
