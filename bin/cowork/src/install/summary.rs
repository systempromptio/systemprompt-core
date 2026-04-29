use super::{
    CredentialsOutcome, InstallSummary, ManagedProfileOutcome, MdmDisplay, UninstallSummary,
    os_label,
};
use crate::config::paths::{self, Scope};

pub fn render_install_summary(s: &InstallSummary) -> String {
    let mut out = String::new();
    out.push_str("Installed systemprompt-cowork integration\n");
    out.push_str(&format!(
        "  org-plugins: {} ({})\n",
        s.location.path.display(),
        match s.location.scope {
            Scope::System => "system-wide",
            Scope::User => "per-user",
        }
    ));
    let meta = paths::metadata_dir(&s.location.path);
    out.push_str(&format!("  metadata:    {}\n", meta.display()));
    out.push_str(&format!(
        "    user.json:    {}\n",
        meta.join(paths::USER_FRAGMENT).display()
    ));
    out.push_str(&format!(
        "    skills/:      {}\n",
        meta.join(paths::SKILLS_DIR).display()
    ));
    out.push_str(&format!(
        "    agents/:      {}\n",
        meta.join(paths::AGENTS_DIR).display()
    ));
    out.push_str(&format!(
        "    managed-mcp:  {}\n",
        meta.join(paths::MANAGED_MCP_FRAGMENT).display()
    ));
    out.push_str(&format!("  binary:      {}\n", s.binary.display()));
    out.push_str(
        "  Run `systemprompt-cowork sync` to populate user identity, skills, agents, and MCP \
         servers.\n",
    );

    render_mdm(&mut out, &s.mdm);

    if let Some(sched) = &s.schedule {
        out.push('\n');
        out.push_str(&format!(
            "--- Schedule template ({}) ---\n",
            os_label(sched.os)
        ));
        out.push_str(&format!("wrote: {}\n", sched.path.display()));
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
            out.push_str(&format!("--- MDM configuration ({}) ---\n", os_label(*os)));
            out.push_str(snippet);
            if !snippet.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("Tip: rerun with --apply to write these keys directly.\n");
        },
        MdmDisplay::Applied { os, lines } => {
            out.push('\n');
            out.push_str(&format!("--- policy applied ({}) ---\n", os_label(*os)));
            for line in lines {
                out.push_str(&format!("  {line}\n"));
            }
        },
        MdmDisplay::MobileconfigApplied { lines } => {
            out.push('\n');
            out.push_str("--- mobileconfig applied (macOS) ---\n");
            for line in lines {
                out.push_str(&format!("  {line}\n"));
            }
        },
    }
}

pub fn render_uninstall_summary(s: &UninstallSummary) -> String {
    let mut out = String::new();
    if let Some(p) = &s.metadata_removed {
        out.push_str(&format!("Removed {}\n", p.display()));
    }
    if let Some(p) = &s.metadata_already_clean {
        out.push_str(&format!(
            "No metadata dir at {} (already clean)\n",
            p.display()
        ));
    }
    match &s.managed_profile {
        ManagedProfileOutcome::Removed(id) => {
            out.push_str(&format!("Removed managed profile {id}\n"));
        },
        ManagedProfileOutcome::NotInstalled(id) => {
            out.push_str(&format!(
                "No managed profile {id} installed (nothing to remove)\n"
            ));
        },
        ManagedProfileOutcome::RemoveFailed(_) | ManagedProfileOutcome::NotApplicable => {},
    }
    match &s.credentials {
        CredentialsOutcome::Purged(p) => {
            out.push_str(&format!("Purged credentials: {}\n", p.display()));
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
