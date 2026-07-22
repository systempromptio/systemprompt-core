use super::sandbox::{Sandbox, argv};
use systemprompt_bridge::cli::run_with_args;

#[test]
fn install_bootstraps_the_user_scoped_org_plugins_tree() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["install"]));
    });

    assert!(
        sb.org_plugins().is_dir(),
        "install creates the org-plugins root at {}",
        sb.org_plugins().display()
    );
    let sentinel = sb.metadata().join("version.json");
    let raw = std::fs::read_to_string(&sentinel).expect("install writes the version sentinel");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("sentinel is JSON");
    assert!(
        parsed["binary"].as_str().is_some_and(|s| !s.is_empty()),
        "sentinel records the installed binary path: {raw}"
    );
    assert!(
        parsed["gateway_url"].is_null(),
        "no --gateway means no recorded gateway: {raw}"
    );
}

#[test]
fn install_with_a_gateway_persists_it_to_config_and_sentinel() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "install",
            "--gateway",
            "http://gateway.invalid:9100",
            "--pubkey",
            "dGVzdC1wdWJrZXk",
        ]));
    });

    let raw = std::fs::read_to_string(sb.metadata().join("version.json")).expect("sentinel");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("sentinel is JSON");
    assert_eq!(
        parsed["gateway_url"].as_str(),
        Some("http://gateway.invalid:9100")
    );

    let cfg = std::fs::read_to_string(
        sb.config
            .path()
            .join("systemprompt")
            .join("systemprompt-bridge.toml"),
    )
    .expect("install persists a config file");
    assert!(
        cfg.contains("http://gateway.invalid:9100"),
        "gateway_url persisted: {cfg}"
    );
    assert!(
        cfg.contains("dGVzdC1wdWJrZXk"),
        "pinned pubkey persisted: {cfg}"
    );
}

#[test]
fn install_rejects_a_malformed_gateway_before_touching_the_filesystem() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["install", "--gateway", "not a url"]));
    });
    assert!(
        !sb.org_plugins().exists(),
        "argument validation runs before bootstrap"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn install_apply_mobileconfig_is_rejected_off_macos() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["install", "--apply-mobileconfig"]));
    });
    assert!(
        sb.org_plugins().is_dir(),
        "the directory bootstrap still runs before the MDM step fails"
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn install_apply_has_no_linux_mdm_format() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["install", "--apply"]));
    });
    assert!(
        sb.metadata().join("version.json").exists(),
        "bootstrap completes even though Linux has no MDM apply"
    );
}

#[test]
fn install_print_mdm_for_another_os_leaves_the_local_tree_bootstrapped() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["install", "--print-mdm", "macos"]));
        let _ = run_with_args(&argv(&["install", "--print-mdm", "windows"]));
    });
    assert!(sb.org_plugins().is_dir());
}

#[test]
fn uninstall_clears_metadata_and_plugin_dirs_but_keeps_credentials() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
        ]));
        let _ = run_with_args(&argv(&["install"]));
        std::fs::create_dir_all(sb.org_plugins().join("acme-plugin")).expect("seed a plugin dir");
        let _ = run_with_args(&argv(&["uninstall"]));
    });

    assert!(
        !sb.metadata().exists(),
        "uninstall removes the metadata dir"
    );
    assert!(
        !sb.org_plugins().join("acme-plugin").exists(),
        "uninstall removes provisioned plugin dirs"
    );
    assert!(
        sb.config
            .path()
            .join("systemprompt")
            .join("systemprompt-bridge.pat")
            .exists(),
        "without --purge the PAT survives"
    );
}

#[test]
fn uninstall_purge_also_removes_the_stored_credential() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&[
            "login",
            "sp-live-testprefix.secretsecretsecretsecretsecret012345",
        ]));
        let _ = run_with_args(&argv(&["install"]));
        let _ = run_with_args(&argv(&["uninstall", "--purge"]));
    });

    assert!(
        !sb.config
            .path()
            .join("systemprompt")
            .join("systemprompt-bridge.pat")
            .exists(),
        "--purge removes the PAT file"
    );
}

#[test]
fn uninstall_on_a_never_installed_machine_is_clean() {
    let sb = Sandbox::new();
    sb.run(|| {
        let _ = run_with_args(&argv(&["uninstall"]));
    });
    assert!(
        !sb.metadata().exists(),
        "nothing to remove, nothing created"
    );
}
