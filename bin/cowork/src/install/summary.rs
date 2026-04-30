use std::fmt::Write as _;

use super::{
    CredentialsOutcome, InstallSummary, ManagedProfileOutcome, MdmDisplay, UninstallSummary,
    os_label,
};
use crate::config::paths::{self, Scope};

#[must_use]
pub fn render_install_summary(s: &InstallSummary) -> String {
    let mut out = String::new();
    out.push_str("Installed systemprompt-cowork integration\n");
    let scope_label = match s.location.scope {
        Scope::System => "system-wide",
        Scope::User => "per-user",
    };
    let _ = writeln!(
        out,
        "  org-plugins: {} ({scope_label})",
        s.location.path.display()
    );
    let meta = paths::metadata_dir(&s.location.path);
    let _ = writeln!(out, "  metadata:    {}", meta.display());
    let _ = writeln!(
        out,
        "    user.json:    {}",
        meta.join(paths::USER_FRAGMENT).display()
    );
    let synthetic = s.location.path.join(paths::SYNTHETIC_PLUGIN_NAME);
    let _ = writeln!(out, "  managed plugin: {}", synthetic.display());
    let _ = writeln!(
        out,
        "    skills/:    {}",
        synthetic.join("skills").display()
    );
    let _ = writeln!(
        out,
        "    agents/:    {}",
        synthetic.join("agents").display()
    );
    let _ = writeln!(
        out,
        "    .mcp.json:  {}",
        synthetic.join(".mcp.json").display()
    );
    let _ = writeln!(out, "  binary:      {}", s.binary.display());
    out.push_str(
        "  Run `systemprompt-cowork sync` to populate user identity, skills, agents, and MCP \
         servers.\n",
    );

    render_mdm(&mut out, &s.mdm);

    if let Some(sched) = &s.schedule {
        out.push('\n');
        let _ = writeln!(out, "--- Schedule template ({}) ---", os_label(sched.os));
        let _ = writeln!(out, "wrote: {}", sched.path.display());
        out.push_str(&sched.install_hint);
        if !sched.install_hint.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

fn render_mdm(out: &mut String, mdm: &MdmDisplay) {
    match mdm {
        MdmDisplay::Snippet { os, snippet } => {
            out.push('\n');
            let _ = writeln!(out, "--- MDM configuration ({}) ---", os_label(*os));
            out.push_str(snippet);
            if !snippet.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("Tip: rerun with --apply to write these keys directly.\n");
        },
        MdmDisplay::Applied { os, lines } => {
            out.push('\n');
            let _ = writeln!(out, "--- policy applied ({}) ---", os_label(*os));
            for line in lines {
                let _ = writeln!(out, "  {line}");
            }
        },
        MdmDisplay::MobileconfigApplied { lines } => {
            out.push('\n');
            out.push_str("--- mobileconfig applied (macOS) ---\n");
            for line in lines {
                let _ = writeln!(out, "  {line}");
            }
        },
    }
}

#[must_use]
pub fn render_uninstall_summary(s: &UninstallSummary) -> String {
    let mut out = String::new();
    if let Some(p) = &s.metadata_removed {
        let _ = writeln!(out, "Removed {}", p.display());
    }
    if let Some(p) = &s.metadata_already_clean {
        let _ = writeln!(out, "No metadata dir at {} (already clean)", p.display());
    }
    match &s.managed_profile {
        ManagedProfileOutcome::Removed(id) => {
            let _ = writeln!(out, "Removed managed profile {id}");
        },
        ManagedProfileOutcome::NotInstalled(id) => {
            let _ = writeln!(out, "No managed profile {id} installed (nothing to remove)");
        },
        ManagedProfileOutcome::RemoveFailed(_) | ManagedProfileOutcome::NotApplicable => {},
    }
    match &s.credentials {
        CredentialsOutcome::Purged(p) => {
            let _ = writeln!(out, "Purged credentials: {}", p.display());
        },
        CredentialsOutcome::Kept => {
            out.push_str(
                "Credentials left intact. Use `systemprompt-cowork uninstall --purge` to also \
                 clear them.\n",
            );
        },
        CredentialsOutcome::PurgeFailed(_) => {},
    }
    out
}
