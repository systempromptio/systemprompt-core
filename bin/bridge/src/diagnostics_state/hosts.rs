//! Diagnostics sections for the org-plugins tree, every host profile, the
//! working directories, the single-instance lock and the update policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::files::{append_path, describe_access};
use crate::config::paths;
use crate::context::BridgeContext;

pub(super) fn append_org_plugins(out: &mut Vec<String>) {
    out.push("org-plugins:".to_owned());
    match paths::org_plugins_effective() {
        Some(location) => {
            out.push(format!(
                "  effective: {} ({:?}, {})",
                location.path.display(),
                location.scope,
                match &location.reason {
                    paths::FallbackReason::Preferred => "preferred".to_owned(),
                    paths::FallbackReason::SystemUnwritable { system_path } =>
                        format!("system path {} unwritable", system_path.display()),
                }
            ));
            append_tree(out, &location.path);
        },
        None => out.push("  <no org-plugins location resolvable>".to_owned()),
    }
    for root in paths::all_known_org_plugins_roots() {
        if root.exists() {
            append_path(out, "known root", &root);
        }
    }
}

fn append_tree(out: &mut Vec<String>, root: &Path) {
    out.push(format!("    {}", describe_access(root)));
    let Ok(entries) = std::fs::read_dir(root) else {
        out.push("    <not listable>".to_owned());
        return;
    };
    let mut children: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    children.sort();
    for child in children {
        let name = child
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        append_path(out, &name, &child);
    }
}

pub(super) fn append_host_profiles(out: &mut Vec<String>, ctx: &BridgeContext) {
    out.push("host profiles:".to_owned());
    let env = crate::integration::host_app::ProbeEnv::new(
        ctx.proxy.loopback(),
        std::sync::Arc::clone(&ctx.start_menu),
    );
    for host in crate::integration::host_apps() {
        let snapshot = host.probe(&env);
        out.push(format!(
            "  {}: {:?}; app {:?}; running {}",
            snapshot.host_id, snapshot.profile_state, snapshot.app_installed, snapshot.host_running
        ));
        for (key, value) in &snapshot.profile_keys {
            let shown = if crate::config::redaction::is_sensitive_key(key) {
                format!("<{} chars>", value.len())
            } else {
                value.clone()
            };
            out.push(format!("    {key} = {shown}"));
        }
        match snapshot.profile_source.as_deref() {
            Some(source) if Path::new(source).exists() => {
                append_path(out, "  profile", Path::new(source));
            },
            Some(source) => out.push(format!("    profile source: {source}")),
            None => out.push("    profile source: <none>".to_owned()),
        }
    }
}

pub(super) fn append_working_dirs(out: &mut Vec<String>) {
    out.push("working dirs:".to_owned());
    let dirs = [
        ("staging", paths::bridge_staging_dir()),
        ("metadata", paths::bridge_metadata_dir()),
    ];
    for (label, dir) in dirs {
        match dir {
            Some(dir) => append_path(out, label, &dir),
            None => out.push(format!("  {label}: <unresolvable>")),
        }
    }
    if let Some(meta) = paths::bridge_metadata_dir() {
        append_path(out, "last sync", &meta.join(paths::LAST_SYNC_SENTINEL));
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn append_single_instance(out: &mut Vec<String>) {
    out.push("single instance: <no GUI lock on this platform>".to_owned());
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(super) fn append_single_instance(out: &mut Vec<String>) {
    out.push("single instance:".to_owned());
    let lock = crate::single_instance::lock_path();
    append_path(out, "lock", &lock);
    let handoff = lock.with_extension("handoff");
    if handoff.exists() {
        append_path(out, "handoff", &handoff);
        if let Ok(body) = std::fs::read_to_string(&handoff) {
            out.push(format!("    {}", body.trim()));
        }
    }
}

pub(super) fn append_update(out: &mut Vec<String>) {
    out.push("update:".to_owned());
    out.push(format!(
        "  policy: {:?}",
        crate::update::auto_update_policy()
    ));
    match crate::config::load() {
        Ok(cfg) => out.push(format!(
            "  gateway: {}",
            crate::config::gateway_url_or_default(&cfg)
        )),
        Err(e) => out.push(format!("  gateway: <config: {e}>")),
    }
}
