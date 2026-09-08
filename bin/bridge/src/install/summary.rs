//! Human-readable install/uninstall summary rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    CredentialsOutcome, InstallSummary, ManagedProfileOutcome, MdmDisplay, ScheduleDisplay,
    ScheduleRemoval, UninstallSummary, os_label,
};
use crate::config::paths::{self, Scope};

#[must_use]
pub fn render_install_summary(s: &InstallSummary) -> String {
    let binary_name = crate::brand::brand().binary_name;
    let scope_label = match s.location.scope {
        Scope::System => "system-wide",
        Scope::User => "per-user",
    };
    let mut lines = vec![
        format!("Installed {binary_name} integration"),
        format!(
            "  org-plugins: {} ({scope_label})",
            s.location.path.display()
        ),
    ];
    if let Some(meta) = paths::bridge_metadata_dir() {
        lines.push(format!("  metadata:    {}", meta.display()));
        lines.push(format!(
            "    user.json:    {}",
            meta.join(paths::USER_FRAGMENT).display()
        ));
    }
    lines.push(format!(
        "  managed plugins: {}/<plugin-id>/",
        s.location.path.display()
    ));
    lines.push(format!("  binary:      {}", s.binary.display()));
    lines.push(format!(
        "  Run `{binary_name} sync` to populate user identity, skills, agents, and MCP servers."
    ));
    lines.extend(mdm_lines(&s.mdm));
    if let Some(sched) = &s.schedule {
        lines.extend(schedule_lines(sched));
    }
    joined(&lines)
}

fn schedule_lines(sched: &ScheduleDisplay) -> Vec<String> {
    match sched {
        ScheduleDisplay::Template(emit) => {
            let mut lines = vec![
                String::new(),
                format!("--- Schedule template ({}) ---", os_label(emit.os)),
                format!("wrote: {}", emit.path.display()),
            ];
            lines.extend(emit.install_hint.lines().map(str::to_owned));
            lines.push("Tip: rerun with --apply-schedule to register it directly.".to_owned());
            lines
        },
        ScheduleDisplay::Applied(applied) => {
            let mut lines = vec![
                String::new(),
                format!(
                    "--- sync schedule registered ({}) ---",
                    os_label(applied.os)
                ),
            ];
            lines.extend(applied.lines.iter().map(|line| format!("  {line}")));
            lines
        },
    }
}

fn mdm_lines(mdm: &MdmDisplay) -> Vec<String> {
    match mdm {
        MdmDisplay::Snippet { os, snippet } => {
            let mut lines = vec![
                String::new(),
                format!("--- MDM configuration ({}) ---", os_label(*os)),
            ];
            lines.extend(snippet.lines().map(str::to_owned));
            lines.push("Tip: rerun with --apply to write these keys directly.".to_owned());
            lines
        },
        MdmDisplay::Applied { os, report } => {
            let mut lines = vec![
                String::new(),
                format!("--- policy applied ({}) ---", os_label(*os)),
            ];
            lines.extend(report.lines.iter().map(|line| format!("  {line}")));
            lines
        },
        MdmDisplay::MobileconfigPrepared { lines: applied } => {
            let mut lines = vec![
                String::new(),
                "--- mobileconfig prepared; approval in System Settings required (macOS) ---"
                    .to_owned(),
            ];
            lines.extend(applied.iter().map(|line| format!("  {line}")));
            lines
        },
    }
}

#[must_use]
pub fn render_uninstall_summary(s: &UninstallSummary) -> String {
    let mut lines = Vec::new();
    if let Some(p) = &s.metadata_removed {
        lines.push(format!("Removed {}", p.display()));
    }
    if let Some(p) = &s.metadata_already_clean {
        lines.push(format!(
            "No metadata dir at {} (already clean)",
            p.display()
        ));
    }
    match &s.managed_profile {
        ManagedProfileOutcome::Removed(id) => {
            lines.push(format!("Removed managed profile {id}"));
        },
        ManagedProfileOutcome::NotInstalled(id) => {
            lines.push(format!(
                "No managed profile {id} installed (nothing to remove)"
            ));
        },
        ManagedProfileOutcome::RemoveFailed(_) | ManagedProfileOutcome::NotApplicable => {},
    }
    match &s.credentials {
        CredentialsOutcome::Purged(p) => {
            lines.push(format!("Purged credentials: {}", p.display()));
        },
        CredentialsOutcome::Kept => {
            lines.push(format!(
                "Credentials left intact. Use `{} uninstall --purge` to also clear them.",
                crate::brand::brand().binary_name
            ));
        },
        CredentialsOutcome::PurgeFailed(_) => {},
    }
    match &s.schedule {
        ScheduleRemoval::Removed(label) => {
            lines.push(format!("Removed scheduled sync job {label}"));
        },
        ScheduleRemoval::NotInstalled(label) if !label.is_empty() => {
            lines.push(format!(
                "No scheduled sync job {label} registered (nothing to remove)"
            ));
        },
        ScheduleRemoval::NotInstalled(_) | ScheduleRemoval::Failed(_) => {},
    }
    joined(&lines)
}

fn joined(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    lines.join("\n") + "\n"
}
