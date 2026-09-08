//! Merging the bridge-owned keys into a Claude Code settings document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::{MdmError, bridge_env, shell_command_for};

pub(super) fn merge_bridge_keys(
    root: &mut serde_json::Map<String, serde_json::Value>,
    settings_path: &Path,
    gateway: &str,
    helper: &Path,
) -> Result<(), MdmError> {
    let conflicts = forced_login_conflicts(root);
    if !conflicts.is_empty() {
        return Err(MdmError::InvalidConfig(conflicts.join("; ")));
    }
    let env = root
        .entry("env".to_owned())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(env) = env.as_object_mut() else {
        return Err(MdmError::EnvNotObject {
            path: settings_path.to_path_buf(),
        });
    };
    env.extend(bridge_env(gateway));
    root.insert(
        "apiKeyHelper".to_owned(),
        serde_json::Value::String(shell_command_for(helper)),
    );
    Ok(())
}

// Why: Claude Code v2.1.146+ forceLoginMethod/forceLoginOrgUUID block API keys
// and apiKeyHelper.
fn forced_login_conflicts(root: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    ["forceLoginMethod", "forceLoginOrgUUID"]
        .into_iter()
        .filter(|key| root.contains_key(*key))
        .map(|key| {
            format!(
                "WARNING: managed settings already set \"{key}\", which blocks the gateway \
                 credential at startup — remove it or Claude Code will refuse to run"
            )
        })
        .collect()
}
