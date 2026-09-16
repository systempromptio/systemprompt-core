//! The `OpenCode` plugin that reports skill use to the gateway.
//!
//! `OpenCode` loads every module in `~/.config/opencode/plugin/`, so the bridge
//! writes one file there and removes it when no plugin owns governance. The
//! file carries the per-plugin hook token for the loopback proxy's track
//! route; the proxy mints the gateway JWT and stamps device identity exactly as
//! it does for Claude Code hooks, so nothing server-side distinguishes the
//! two hosts except the `skill_ref`, which resolves the `OpenCode` skill
//! directory back to its `plugin:skill` identity. The plugin also stamps
//! every chat request with a v5 session UUID derived under the namespace
//! substituted from [`crate::feedback::opencode_session`], so the proxy can
//! bind the request to the same context as the hook events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::gateway::manifest::SignedManifest;
use crate::host_sync::ApplyError;
use crate::integration::managed_skills::kebab_dir;
use crate::proxy::LoopbackEndpoint;

const TEMPLATE: &str = include_str!("systemprompt-hooks.js");
const FILE_NAME: &str = "systemprompt-hooks.js";

fn plugin_path() -> PathBuf {
    super::super::config::user_dir()
        .join("plugin")
        .join(FILE_NAME)
}

fn skill_map(manifest: &SignedManifest) -> BTreeMap<String, String> {
    manifest
        .skills
        .iter()
        .filter(|skill| skill.hosts.is_empty() || skill.hosts.iter().any(|h| h == "opencode"))
        .map(|skill| {
            let dir = kebab_dir(skill.id.as_str());
            let owner = skill
                .plugins
                .first()
                .map_or_else(|| "opencode".to_owned(), |p| p.as_str().to_owned());
            (dir.clone(), format!("{owner}:{dir}"))
        })
        .collect()
}

pub(super) fn write_hook_plugin(
    loopback: &LoopbackEndpoint,
    manifest: &SignedManifest,
) -> Result<(), ApplyError> {
    let Some(plugin) = manifest.plugins.iter().find(|p| p.hooks.governance) else {
        return remove_hook_plugin();
    };
    let authorization = loopback
        .hook_bearer(&plugin.id)
        .map_err(|source| ApplyError::Io {
            context: format!("loopback hook token for OpenCode plugin ({})", plugin.id),
            source,
        })?;
    let map =
        serde_json::to_string(&skill_map(manifest)).map_err(|source| ApplyError::Serialize {
            what: "OpenCode skill map".to_owned(),
            source,
        })?;
    let body = TEMPLATE
        .replace(
            "__TRACK_URL__",
            &format!(
                "{}/api/public/hooks/track?plugin_id={}",
                loopback.origin(),
                plugin.id
            ),
        )
        .replace("__AUTHORIZATION__", &authorization)
        .replace("__SKILL_MAP__", &map)
        .replace(
            "__SESSION_NAMESPACE__",
            &crate::feedback::opencode_session::OPENCODE_SESSION_NAMESPACE
                .hyphenated()
                .to_string(),
        );
    let path = plugin_path();
    if std::fs::read(&path).is_ok_and(|current| current == body.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ApplyError::Io {
            context: format!("create {}", parent.display()),
            source,
        })?;
    }
    crate::fsutil::atomic_write_0644(&path, body.as_bytes()).map_err(|source| ApplyError::Io {
        context: format!("write {}", path.display()),
        source,
    })
}

pub(crate) fn remove_hook_plugin() -> Result<(), ApplyError> {
    let path = plugin_path();
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ApplyError::Io {
            context: format!("remove {}", path.display()),
            source,
        }),
    }
}
