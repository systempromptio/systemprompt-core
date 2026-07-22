#![allow(clippy::all)]

// Pins SP_BRIDGE_ORG_PLUGINS_SYSTEM to an unwritable path inside the sandbox
// so path resolution can never adopt the host's real system root (on CI
// runners /opt is writable and other suites may have provisioned it).
#[cfg(test)]
fn unwritable_system_org_plugins(base: &std::path::Path) -> std::ffi::OsString {
    let root = base.join("system-root");
    std::fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    root.join("Claude").join("org-plugins").into()
}

#[cfg(test)]
mod apply;
#[cfg(test)]
mod hosts;
#[cfg(test)]
mod manifest_verify;
#[cfg(test)]
mod replay_gate;
