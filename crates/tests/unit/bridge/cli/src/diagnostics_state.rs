use std::path::Path;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use tempfile::TempDir;

fn sandboxed<T>(home: &Path, extra: Vec<(&str, Option<String>)>, run: impl FnOnce() -> T) -> T {
    let root = home.display().to_string();
    let mut vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
        ("XDG_STATE_HOME", Some(format!("{root}/.state"))),
        ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
        ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_CONFIG", None),
        ("SUDO_USER", None),
    ];
    vars.extend(extra);
    temp_env::with_vars(vars, run)
}

fn render_in(home: &Path, extra: Vec<(&str, Option<String>)>) -> String {
    sandboxed(home, extra, || {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach context");
        systemprompt_bridge::diagnostics_state::render(&ctx)
    })
}

#[test]
fn the_dump_carries_every_section_a_broken_proxy_needs_explained() {
    let home = TempDir::new().expect("home");
    let out = render_in(home.path(), Vec::new());

    for section in [
        "proxy:",
        "  install id: ",
        "  role:       ",
        "  endpoint:   ",
        "  secret fp:  ",
        "port file:",
        "config dir:",
        "org-plugins:",
        "host profiles:",
        "working dirs:",
        "single instance",
        "update:",
        "claude desktop policy:",
        "bridge processes:",
    ] {
        assert!(
            out.contains(section),
            "diagnostics dump lost {section}\n{out}"
        );
    }
    assert!(out.ends_with('\n'));
}

#[test]
fn an_attached_process_says_it_does_not_serve_and_names_the_port_holder() {
    let home = TempDir::new().expect("home");
    let out = render_in(home.path(), Vec::new());

    assert!(
        out.contains("attached (this process does not serve)"),
        "an attach-mode context must not claim to be serving\n{out}"
    );
    assert!(
        out.contains("port 48217: "),
        "the default proxy port must always be probed\n{out}"
    );
}

#[test]
fn a_writable_system_org_plugins_tree_is_listed_entry_by_entry() {
    let home = TempDir::new().expect("home");
    let system = home.path().join("system-org-plugins");
    std::fs::create_dir_all(system.join("acme-plugin")).expect("plugin dir");
    std::fs::write(system.join("marketplace.json"), b"{\"plugins\":[]}").expect("marker file");

    let out = render_in(
        home.path(),
        vec![(
            "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
            Some(system.display().to_string()),
        )],
    );

    assert!(out.contains("(System, preferred)"), "{out}");
    assert!(out.contains("marketplace.json: "), "{out}");
    assert!(out.contains("acme-plugin: "), "{out}");
    assert!(
        out.contains("14 bytes readable"),
        "a listed file must report its size and readability\n{out}"
    );
    assert!(
        out.contains("dir listable"),
        "a listed directory must report that it can be enumerated\n{out}"
    );
}

#[cfg(unix)]
#[test]
fn an_unwritable_system_org_plugins_tree_falls_back_to_the_user_tree_and_says_why() {
    use std::os::unix::fs::PermissionsExt;

    if unsafe { libc::geteuid() } == 0 {
        return;
    }

    let home = TempDir::new().expect("home");
    let system = home.path().join("readonly-system-tree");
    std::fs::create_dir_all(&system).expect("system tree");
    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o555)).expect("lock");

    let out = render_in(
        home.path(),
        vec![(
            "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
            Some(system.display().to_string()),
        )],
    );

    std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o755)).expect("unlock");

    assert!(out.contains("(User, "), "{out}");
    assert!(
        out.contains(&format!("system path {} unwritable", system.display())),
        "the fallback must name the system path it could not use\n{out}"
    );
}

#[cfg(unix)]
#[test]
fn an_unlistable_org_plugins_tree_is_reported_rather_than_silently_empty() {
    use std::os::unix::fs::PermissionsExt;

    if unsafe { libc::geteuid() } == 0 {
        return;
    }

    let home = TempDir::new().expect("home");
    let system = home.path().join("system-org-plugins");
    std::fs::create_dir_all(&system).expect("plugin root");
    let locked = system.join("locked");
    std::fs::create_dir_all(&locked).expect("locked dir");
    std::fs::write(locked.join("inner.json"), b"{}").expect("inner file");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("lock dir");

    let out = render_in(
        home.path(),
        vec![(
            "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
            Some(system.display().to_string()),
        )],
    );

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("unlock dir");

    assert!(
        out.contains("locked: ") && out.contains("UNLISTABLE:"),
        "a directory the account cannot enumerate must be called out\n{out}"
    );
}

#[cfg(unix)]
#[test]
fn every_described_path_carries_its_unix_mode_and_ownership() {
    let home = TempDir::new().expect("home");
    let system = home.path().join("system-org-plugins");
    std::fs::create_dir_all(&system).expect("plugin root");
    std::fs::write(system.join("manifest.json"), b"{}").expect("file");

    let out = render_in(
        home.path(),
        vec![(
            "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
            Some(system.display().to_string()),
        )],
    );

    assert!(
        out.contains(&format!("uid {}", unsafe { libc::geteuid() })),
        "the dump must record who owns each path\n{out}"
    );
    assert!(out.contains("mode 6"), "{out}");
}

#[test]
fn host_profiles_and_working_dirs_are_reported_even_when_nothing_is_installed() {
    let home = TempDir::new().expect("home");
    let out = render_in(home.path(), Vec::new());

    let hosts = out
        .split("host profiles:")
        .nth(1)
        .expect("a host profiles section")
        .split("\nworking dirs:")
        .next()
        .expect("the section ends at working dirs");

    assert!(
        hosts.contains("app "),
        "each host row must state whether the app is installed\n{out}"
    );
    assert!(
        hosts.contains("running "),
        "each host row must state whether the host is running\n{out}"
    );
    assert!(
        hosts.contains("profile source: <none>") || hosts.contains("  profile: "),
        "each host must report where its profile came from\n{out}"
    );
    assert!(out.contains("staging: "), "{out}");
    assert!(out.contains("metadata: "), "{out}");
    assert!(out.contains("last sync: "), "{out}");
}

#[test]
fn the_update_section_reports_the_policy_and_the_configured_gateway() {
    let home = TempDir::new().expect("home");
    let out = render_in(home.path(), Vec::new());

    let update = out
        .split("update:\n")
        .nth(1)
        .expect("an update section")
        .to_owned();

    assert!(update.contains("  policy: "), "{out}");
    assert!(
        update.contains("  gateway: http"),
        "the dump must name the gateway the bridge would talk to\n{out}"
    );
}

#[test]
fn a_secret_bearing_host_profile_value_is_replaced_by_its_length() {
    let home = TempDir::new().expect("home");
    let out = render_in(home.path(), Vec::new());

    assert!(
        !out.contains("Bearer "),
        "a diagnostics dump must never carry a bearer token\n{out}"
    );
}
