use crate::config::paths;

use super::Check;

// Why: every sync needs to create a staging dir and write a metadata sentinel.
// Both live under `%LOCALAPPDATA%\systemprompt-bridge\` (or per-OS equivalent)
// — always user-writable by design. If the dirs can't be created, the bridge
// is either running with a stripped/unset LOCALAPPDATA or a sandbox restriction
// applied. The error from sync would be `io error in create staging: …` —
// this check surfaces the same condition pre-emptively with a recovery hint.
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
        // Why: best-effort removal of the write probe; a leftover zero-byte file
        // is harmless and a removal failure is not actionable here.
        _ = std::fs::remove_file(&probe);
    }
    Check::ok(
        "bridge working dir",
        format!(
            "staging+metadata writable under {}",
            paths::bridge_working_dir()
                .map_or_else(|| "<unresolved>".to_string(), |p| p.display().to_string())
        ),
    )
}

// Why: every sync needs to write the per-plugin tree at
// `<org_plugins_root>/<plugin-id>/`. On Windows the root is
// `C:\Program Files\Claude\org-plugins\` which is admin-write-only by default.
// `install --apply` widens its ACL once (icacls /grant <user>:(OI)(CI)M) so
// unelevated syncs can update plugin contents. If that grant isn't present
// (admin reset the ACL, or install --apply was never run), every sync fails
// with `Access is denied`. Catch it here with the actionable recovery hint
// rather than the user staring at a generic OS-error in the GUI.
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
            // Why: best-effort probe cleanup; failure leaves a harmless empty
            // file and is not actionable.
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
