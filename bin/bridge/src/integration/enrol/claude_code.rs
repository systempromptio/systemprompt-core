//! Enrolling the Claude Code CLI, which has no host app: its inference reaches
//! the gateway only when its settings file names the loopback proxy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;

use super::{Outcome, Report};
use crate::context::BridgeContext;
use crate::integration::sync_only::{SyncOnlyAgent, sync_only_agent};

pub(super) const LABEL: &str = "gateway keys merged into Claude Code's settings file";

#[must_use]
pub fn installed_agent() -> Option<&'static SyncOnlyAgent> {
    if crate::integration::claude_code_cli::claude_cli_installed() {
        sync_only_agent(HostKind::ClaudeCode)
    } else {
        None
    }
}

#[must_use]
pub fn enrol_report(bridge: &BridgeContext) -> Report {
    report(enrol(bridge))
}

pub(super) fn not_enabled_report() -> Report {
    report(Outcome::NotEnabled)
}

pub(super) fn removal_report() -> Report {
    report(remove())
}

fn report(outcome: Outcome) -> Report {
    Report {
        host_id: HostKind::ClaudeCode,
        display_name: sync_only_agent(HostKind::ClaudeCode)
            .map_or("Claude Code", |agent| agent.display_name),
        install_action_label: LABEL,
        outcome,
        warnings: Vec::new(),
    }
}

fn enrol(bridge: &BridgeContext) -> Outcome {
    let gateway = bridge.proxy.loopback().origin();
    match crate::install::mdm::claude_code_settings::apply_managed_settings(&gateway) {
        Ok(report) => {
            for line in report.lines {
                tracing::info!(target: "bridge::install", detail = %line, "claude code settings");
            }
            Outcome::Installed
        },
        Err(e) => Outcome::Failed(e.into()),
    }
}

fn remove() -> Outcome {
    match crate::install::mdm::claude_code_settings::remove_managed_settings() {
        Ok(lines) if lines.is_empty() => Outcome::NothingToRemove,
        Ok(_) => Outcome::Removed,
        Err(e) => Outcome::Failed(e.into()),
    }
}
