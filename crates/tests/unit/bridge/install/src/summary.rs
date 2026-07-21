use systemprompt_bridge::config::paths::{OrgPluginsLocation, Scope};
use systemprompt_bridge::install::{
    CredentialsOutcome, InstallSummary, ManagedProfileOutcome, MdmDisplay, ScheduleApplied,
    ScheduleDisplay, ScheduleEmit, ScheduleRemoval, UninstallSummary, render_install_summary,
    render_uninstall_summary,
};
use systemprompt_bridge::schedule::Os;

#[test]
fn install_summary_with_mdm_snippet_renders_all_parts() {
    let s = InstallSummary {
        location: OrgPluginsLocation {
            path: "/opt/plugins".into(),
            scope: Scope::User,
        },
        binary: "/usr/bin/systemprompt-bridge".into(),
        mdm: MdmDisplay::Snippet {
            os: Os::Mac,
            snippet: "KEY=val".into(),
        },
        schedule: None,
    };
    let out = render_install_summary(&s);
    assert!(out.contains("Installed systemprompt-bridge integration"));
    assert!(out.contains("/opt/plugins"));
    assert!(out.contains("per-user"));
    assert!(out.contains("/usr/bin/systemprompt-bridge"));
    assert!(out.contains("MDM configuration"));
    assert!(out.contains("KEY=val"));
    assert!(out.contains("--apply"));
}

#[test]
fn install_summary_with_applied_policy_renders_lines() {
    let s = InstallSummary {
        location: OrgPluginsLocation {
            path: "/opt/plugins".into(),
            scope: Scope::User,
        },
        binary: "/usr/bin/systemprompt-bridge".into(),
        mdm: MdmDisplay::Applied {
            os: Os::Windows,
            lines: vec!["wrote HKCU".into()],
        },
        schedule: None,
    };
    let out = render_install_summary(&s);
    assert!(out.contains("policy applied"));
    assert!(out.contains("wrote HKCU"));
}

#[test]
fn install_summary_with_schedule_renders_template() {
    let s = InstallSummary {
        location: OrgPluginsLocation {
            path: "/opt/plugins".into(),
            scope: Scope::User,
        },
        binary: "/usr/bin/systemprompt-bridge".into(),
        mdm: MdmDisplay::Snippet {
            os: Os::Mac,
            snippet: "KEY=val".into(),
        },
        schedule: Some(ScheduleDisplay::Template(ScheduleEmit {
            os: Os::Linux,
            path: "/tmp/x.timer".into(),
            install_hint: "systemctl enable".into(),
        })),
    };
    let out = render_install_summary(&s);
    assert!(out.contains("Schedule template"));
    assert!(out.contains("/tmp/x.timer"));
    assert!(out.contains("systemctl enable"));
}

#[test]
fn install_summary_system_scope_renders_system_wide() {
    let s = InstallSummary {
        location: OrgPluginsLocation {
            path: "/opt/plugins".into(),
            scope: Scope::System,
        },
        binary: "/usr/bin/systemprompt-bridge".into(),
        mdm: MdmDisplay::Snippet {
            os: Os::Mac,
            snippet: "KEY=val".into(),
        },
        schedule: None,
    };
    let out = render_install_summary(&s);
    assert!(out.contains("system-wide"));
}

#[test]
fn uninstall_summary_removed_and_kept() {
    let s = UninstallSummary {
        metadata_removed: Some("/x".into()),
        metadata_already_clean: None,
        managed_profile: ManagedProfileOutcome::Removed("profile-id"),
        credentials: CredentialsOutcome::Kept,
        schedule: ScheduleRemoval::Removed("io.systemprompt.bridge-sync".into()),
    };
    let out = render_uninstall_summary(&s);
    assert!(out.contains("Removed /x"));
    assert!(out.contains("Removed managed profile profile-id"));
    assert!(out.contains("left intact"));
    assert!(out.contains("Removed scheduled sync job io.systemprompt.bridge-sync"));
}

#[test]
fn uninstall_summary_purged_and_not_installed() {
    let s = UninstallSummary {
        metadata_removed: None,
        metadata_already_clean: None,
        managed_profile: ManagedProfileOutcome::NotInstalled("pid"),
        credentials: CredentialsOutcome::Purged("/creds".into()),
        schedule: ScheduleRemoval::NotInstalled("io.systemprompt.bridge-sync".into()),
    };
    let out = render_uninstall_summary(&s);
    assert!(out.contains("Purged credentials"));
    assert!(out.contains("No managed profile pid installed"));
    assert!(out.contains("No scheduled sync job io.systemprompt.bridge-sync registered"));
}

#[test]
fn install_summary_with_applied_schedule_renders_registration() {
    let s = InstallSummary {
        location: OrgPluginsLocation {
            path: "/opt/plugins".into(),
            scope: Scope::User,
        },
        binary: "/usr/bin/systemprompt-bridge".into(),
        mdm: MdmDisplay::Snippet {
            os: Os::Mac,
            snippet: "KEY=val".into(),
        },
        schedule: Some(ScheduleDisplay::Applied(ScheduleApplied {
            os: Os::Linux,
            label: "systemprompt-bridge-sync".into(),
            path: "/home/u/.config/systemd/user/systemprompt-bridge-sync.timer".into(),
            lines: vec!["systemd user timer: systemprompt-bridge-sync.timer".into()],
        })),
    };
    let out = render_install_summary(&s);
    assert!(out.contains("sync schedule registered"));
    assert!(out.contains("systemprompt-bridge-sync.timer"));
    assert!(!out.contains("Schedule template"));
}
