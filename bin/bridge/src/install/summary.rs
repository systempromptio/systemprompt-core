//! Human-readable install/uninstall summary rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Write as _;

use super::{
    CredentialsOutcome, InstallSummary, ManagedProfileOutcome, MdmDisplay, ScheduleDisplay,
    ScheduleRemoval, UninstallSummary, os_label,
};
use crate::config::paths::{self, Scope};

#[must_use]
pub fn render_install_summary(s: &InstallSummary) -> String {
    let mut out = String::new();
    _ = writeln!(
        out,
        "Installed {} integration",
        crate::brand::brand().binary_name
    );
    let scope_label = match s.location.scope {
        Scope::System => "system-wide",
        Scope::User => "per-user",
    };
    _ = writeln!(
        out,
        "  org-plugins: {} ({scope_label})",
        s.location.path.display()
    );
    if let Some(meta) = paths::bridge_metadata_dir() {
        _ = writeln!(out, "  metadata:    {}", meta.display());
        _ = writeln!(
            out,
            "    user.json:    {}",
            meta.join(paths::USER_FRAGMENT).display()
        );
    }
    _ = writeln!(
        out,
        "  managed plugins: {}/<plugin-id>/",
        s.location.path.display()
    );
    _ = writeln!(out, "  binary:      {}", s.binary.display());
    _ = writeln!(
        out,
        "  Run `{} sync` to populate user identity, skills, agents, and MCP servers.",
        crate::brand::brand().binary_name
    );

    render_mdm(&mut out, &s.mdm);

    if let Some(sched) = &s.schedule {
        render_schedule(&mut out, sched);
    }
    out
}

fn render_schedule(out: &mut String, sched: &ScheduleDisplay) {
    match sched {
        ScheduleDisplay::Template(emit) => {
            out.push('\n');
            _ = writeln!(out, "--- Schedule template ({}) ---", os_label(emit.os));
            _ = writeln!(out, "wrote: {}", emit.path.display());
            out.push_str(&emit.install_hint);
            if !emit.install_hint.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("Tip: rerun with --apply-schedule to register it directly.\n");
        },
        ScheduleDisplay::Applied(applied) => {
            out.push('\n');
            _ = writeln!(
                out,
                "--- sync schedule registered ({}) ---",
                os_label(applied.os)
            );
            for line in &applied.lines {
                _ = writeln!(out, "  {line}");
            }
        },
    }
}

fn render_mdm(out: &mut String, mdm: &MdmDisplay) {
    match mdm {
        MdmDisplay::Snippet { os, snippet } => {
            out.push('\n');
            _ = writeln!(out, "--- MDM configuration ({}) ---", os_label(*os));
            out.push_str(snippet);
            if !snippet.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("Tip: rerun with --apply to write these keys directly.\n");
        },
        MdmDisplay::Applied { os, lines } => {
            out.push('\n');
            _ = writeln!(out, "--- policy applied ({}) ---", os_label(*os));
            for line in lines {
                _ = writeln!(out, "  {line}");
            }
        },
        MdmDisplay::MobileconfigApplied { lines } => {
            out.push('\n');
            out.push_str("--- mobileconfig applied (macOS) ---\n");
            for line in lines {
                _ = writeln!(out, "  {line}");
            }
        },
    }
}

#[must_use]
pub fn render_uninstall_summary(s: &UninstallSummary) -> String {
    let mut out = String::new();
    if let Some(p) = &s.metadata_removed {
        _ = writeln!(out, "Removed {}", p.display());
    }
    if let Some(p) = &s.metadata_already_clean {
        _ = writeln!(out, "No metadata dir at {} (already clean)", p.display());
    }
    match &s.managed_profile {
        ManagedProfileOutcome::Removed(id) => {
            _ = writeln!(out, "Removed managed profile {id}");
        },
        ManagedProfileOutcome::NotInstalled(id) => {
            _ = writeln!(out, "No managed profile {id} installed (nothing to remove)");
        },
        ManagedProfileOutcome::RemoveFailed(_) | ManagedProfileOutcome::NotApplicable => {},
    }
    match &s.credentials {
        CredentialsOutcome::Purged(p) => {
            _ = writeln!(out, "Purged credentials: {}", p.display());
        },
        CredentialsOutcome::Kept => {
            _ = writeln!(
                out,
                "Credentials left intact. Use `{} uninstall --purge` to also clear them.",
                crate::brand::brand().binary_name
            );
        },
        CredentialsOutcome::PurgeFailed(_) => {},
    }
    match &s.schedule {
        ScheduleRemoval::Removed(label) => {
            _ = writeln!(out, "Removed scheduled sync job {label}");
        },
        ScheduleRemoval::NotInstalled(label) if !label.is_empty() => {
            _ = writeln!(
                out,
                "No scheduled sync job {label} registered (nothing to remove)"
            );
        },
        ScheduleRemoval::NotInstalled(_) | ScheduleRemoval::Failed(_) => {},
    }
    out
}
