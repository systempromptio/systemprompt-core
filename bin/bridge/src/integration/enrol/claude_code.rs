//! Enrolling the Claude Code CLI, which has no host app: its inference reaches
//! the gateway only when its settings file names the loopback proxy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Outcome;
#[cfg(unix)]
use crate::context::BridgeContext;

pub(super) const ID: &str = "claude-code";
pub(super) const LABEL: &str = "gateway keys merged into Claude Code's settings file";

#[cfg(unix)]
pub(super) fn enrol(bridge: &BridgeContext) -> Outcome {
    let Some(key_path) = crate::proxy::secret::secret_path() else {
        return Outcome::Failed("the loopback secret path could not be resolved".to_owned());
    };
    let gateway = bridge.proxy.loopback().origin();
    match crate::install::mdm::claude_code_settings::apply_managed_settings(&gateway, &key_path) {
        Ok(report) => {
            for line in report.lines {
                tracing::info!(target: "bridge::install", detail = %line, "claude code settings");
            }
            Outcome::Installed
        },
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(not(unix))]
pub(super) const fn enrol(_bridge: &crate::context::BridgeContext) -> Outcome {
    Outcome::SyncOnly
}

#[cfg(unix)]
pub(super) fn remove() -> Outcome {
    match crate::install::mdm::claude_code_settings::remove_managed_settings() {
        Ok(lines) if lines.is_empty() => Outcome::NothingToRemove,
        Ok(_) => Outcome::Removed,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(not(unix))]
pub(super) const fn remove() -> Outcome {
    Outcome::SyncOnly
}
