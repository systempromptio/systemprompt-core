use crate::config::paths;

use super::Check;

pub(super) fn check_bridge_working_dir() -> Check {
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
        _ = std::fs::remove_file(&probe);
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

// The Windows org-plugins root is admin-write-only by default; `install
// --apply` widens its ACL once so unelevated syncs can write. Without that grant
// every sync fails with `Access is denied`, which this check surfaces early.
pub(super) fn check_org_plugins_writable() -> Check {
    let Some(loc) = paths::org_plugins_effective() else {
        return Check::warn("org-plugins writable", "no org-plugins location resolvable");
    };
    if !loc.path.exists() {
        return Check::warn(
            "org-plugins writable",
            format!(
                "{} not present — run `systemprompt-bridge install --apply`",
                loc.path.display()
            ),
        );
    }
    let probe = loc.path.join(".sp-bridge-writeprobe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            _ = std::fs::remove_file(&probe);
            Check::ok(
                "org-plugins writable",
                format!("{} is writable by the current user", loc.path.display()),
            )
        },
        Err(e) => Check::fail(
            "org-plugins writable",
            format!(
                "{} is NOT writable by the current user ({e}) — re-run `systemprompt-bridge \
                 install --apply` to restore the user-Modify ACL grant (will prompt for UAC)",
                loc.path.display()
            ),
        ),
    }
}
