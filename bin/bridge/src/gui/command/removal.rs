//! How the running bridge was installed, so the UI can show the matching
//! removal instructions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use super::CommandOutcome;
use crate::wire::ipc::BridgeError;

pub(super) fn guidance() -> CommandOutcome {
    match crate::update::installed_path() {
        Ok(path) => {
            let method = method(&path, std::env::consts::OS);
            CommandOutcome::Sync(Ok(json!({
                "method": method,
                "path": path.display().to_string(),
            })))
        },
        Err(e) => CommandOutcome::Sync(Err(BridgeError::internal(e.to_string()))),
    }
}

pub fn method(path: &std::path::Path, platform: &str) -> &'static str {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    if platform == "windows" && normalized.contains("/scoop/apps/bridge/") {
        "scoop"
    } else if platform == "macos" && path.extension().is_some_and(|e| e == "app") {
        "macos"
    } else {
        "standalone"
    }
}
