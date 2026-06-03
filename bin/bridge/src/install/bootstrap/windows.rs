#![cfg(not(unix))]

use std::path::Path;
use std::process::Command;

pub(super) const fn chown_to_sudo_user_if_root(_path: &Path) {}

// Cowork reads org-plugins from `C:\Program Files\Claude\org-plugins`. Windows
// gives `Program Files` an admin-write-only ACL by convention — fine for
// Cowork (read-only) but the bridge needs to publish plugin trees there on
// every sync, and the bridge GUI runs unelevated.
//
// During the (elevated) `install --apply`, grant the current interactive user
// `Modify` on the org-plugins root, inheriting to children (OI/CI). Subsequent
// unelevated `bridge sync` invocations can then write plugin contents in
// place. Bridge-internal scratch (staging, sync sentinels) lives elsewhere
// under `%LOCALAPPDATA%\systemprompt-bridge\` and never touches Program Files.
pub(super) fn grant_user_modify(path: &Path) -> std::io::Result<()> {
    let user =
        std::env::var("USERNAME").map_err(|_| std::io::Error::other("USERNAME env var not set"))?;
    let path_str = path.to_string_lossy().into_owned();
    let grant_arg = format!("{user}:(OI)(CI)M");

    let output = Command::new("icacls")
        .arg(&path_str)
        .arg("/grant:r")
        .arg(&grant_arg)
        .arg("/T")
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::other(format!(
            "icacls grant failed (exit {:?}): {}",
            output.status.code(),
            stderr.trim()
        )));
    }
    Ok(())
}
