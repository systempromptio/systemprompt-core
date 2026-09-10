//! Doctor checks for filesystem layout and permissions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::paths;

use super::Check;

pub fn check_bridge_working_dir() -> Check {
    let Some(staging) = paths::bridge_staging_dir() else {
        return Check::fail(
            "bridge working dir",
            "could not resolve LOCALAPPDATA / state dir — bridge_working_dir() returned None",
        );
    };
    let Some(meta) = paths::bridge_metadata_dir() else {
        return Check::fail(
            "bridge working dir",
            "could not resolve LOCALAPPDATA / state dir for metadata",
        );
    };
    for (label, dir) in [("staging", &staging), ("metadata", &meta)] {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return Check::fail(
                "bridge working dir",
                format!("cannot create {label} at {}: {e}", dir.display()),
            );
        }
        let probe = dir.join(".sp-bridge-writeprobe");
        if let Err(e) = std::fs::write(&probe, b"") {
            return Check::fail(
                "bridge working dir",
                format!(
                    "cannot write {label} at {} — sync will fail with `Access is denied`: {e}",
                    dir.display()
                ),
            );
        }
        crate::fsutil::remove_leftover_file(&probe);
    }
    Check::ok(
        "bridge working dir",
        format!(
            "staging+metadata writable under {}",
            paths::bridge_working_dir()
                .map_or_else(|| "<unresolved>".to_owned(), |p| p.display().to_string())
        ),
    )
}

pub fn check_private_files() -> Check {
    let Some(config) = crate::config::config_path() else {
        return Check::fail("private files", "no config dir resolvable");
    };
    let Some(dir) = config.parent().map(std::path::Path::to_path_buf) else {
        return Check::fail("private files", "config path has no parent");
    };
    let mut candidates = vec![config];
    candidates.extend(crate::proxy::secret::secret_path());
    candidates.extend(crate::proxy::identity::install_id_path());
    candidates.extend(crate::proxy::portfile::portfile_path());
    candidates.push(dir.join(crate::brand::brand().pat_file));
    let mut unreadable = Vec::new();
    for path in candidates.into_iter().filter(|p| p.exists()) {
        if let Err(e) = std::fs::File::open(&path) {
            unreadable.push(format!(
                "{} ({e}; {})",
                path.display(),
                access_detail(&path)
            ));
        }
    }
    if unreadable.is_empty() {
        return Check::ok(
            "private files",
            format!(
                "every private file under {} opens for this user",
                dir.display()
            ),
        );
    }
    Check::fail(
        "private files",
        format!(
            "{} — start the bridge once to repair a file this user owns, or delete the file as \
             an administrator and start the bridge again",
            unreadable.join("; ")
        ),
    )
}

#[cfg(target_os = "windows")]
fn access_detail(path: &std::path::Path) -> String {
    crate::windows_acl::describe(path).unwrap_or_else(|e| format!("acl: <{e}>"))
}

#[cfg(not(target_os = "windows"))]
fn access_detail(path: &std::path::Path) -> String {
    std::fs::metadata(path).map_or_else(|e| format!("<{e}>"), |m| format!("{:?}", m.permissions()))
}

pub fn check_org_plugins_writable() -> Check {
    let Some(loc) = paths::org_plugins_effective() else {
        return Check::warn("org-plugins writable", "no org-plugins location resolvable");
    };
    if !loc.path.exists() {
        return Check::warn(
            "org-plugins writable",
            format!(
                "{} not present — run `{} install --apply`",
                loc.path.display(),
                crate::brand::brand().binary_name
            ),
        );
    }
    let probe = loc.path.join(".sp-bridge-writeprobe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            crate::fsutil::remove_leftover_file(&probe);
            Check::ok(
                "org-plugins writable",
                format!("{} is writable by the current user", loc.path.display()),
            )
        },
        Err(e) => Check::fail(
            "org-plugins writable",
            format!(
                "{} is NOT writable by the current user ({e}) — re-run `{} \
                 install --apply` to restore the user-Modify ACL grant (will prompt for UAC)",
                loc.path.display(),
                crate::brand::brand().binary_name
            ),
        ),
    }
}
