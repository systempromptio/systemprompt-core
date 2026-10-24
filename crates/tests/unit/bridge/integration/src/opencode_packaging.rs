//! `OpenCode` ships as a terminal agent and an Electron desktop app over one
//! user config. The two hosts must tell their own packaging apart and share
//! `opencode.json` without either overwriting the other.

use std::path::Path;
use systemprompt_bridge::gateway::model_view::host_client;
use systemprompt_models::providers::ClientFamily;

use systemprompt_bridge::integration::enrol::retained_for_sibling;
use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, HostApp, ProfileState,
};
use systemprompt_bridge::integration::opencode::{OPENCODE_APP_HOST, OPENCODE_CLI_HOST};
use systemprompt_bridge::sysproc::classify_process;
use systemprompt_models::bridge::host::{HostAppKind, HostKind};
use tempfile::TempDir;

use crate::support::{inputs_for, probe_env};

fn sandbox<R>(cli_on_path: bool, f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin dir");
    if cli_on_path {
        std::fs::write(bin.join("opencode"), "#!/bin/sh\n").expect("stub binary");
    }
    let config_home = root.path().join("config");
    let user = config_home.join("opencode");
    std::fs::create_dir_all(&user).expect("user dir");
    let bridge_dir = config_home.join("systemprompt");
    std::fs::create_dir_all(&bridge_dir).expect("bridge config dir");
    std::fs::write(
        bridge_dir.join("systemprompt-bridge.toml"),
        format!(
            "[opencode]\nmanaged_dir = '{}'\n",
            root.path().join("managed").display()
        ),
    )
    .expect("bridge config");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        (
            "XDG_DATA_HOME",
            Some(root.path().join("data").display().to_string()),
        ),
        ("SP_BRIDGE_CONFIG", None),
        ("PATH", Some(bin.display().to_string())),
        ("LOCALAPPDATA", None),
        ("APPDATA", None),
    ];
    let out = temp_env::with_vars(vars, || f(&user.join("opencode.json")));
    drop(root);
    out
}

#[test]
fn the_two_hosts_are_one_product_in_two_packagings() {
    assert_eq!(OPENCODE_APP_HOST.id(), HostKind::OpenCode);
    assert_eq!(OPENCODE_CLI_HOST.id(), HostKind::OpenCodeCli);
    assert_eq!(OPENCODE_APP_HOST.kind(), HostAppKind::DesktopApp);
    assert_eq!(OPENCODE_CLI_HOST.kind(), HostAppKind::CliTool);
    assert_eq!(OPENCODE_APP_HOST.display_name(), "OpenCode");
    assert_eq!(OPENCODE_CLI_HOST.display_name(), "OpenCode CLI");
    assert_eq!(OPENCODE_APP_HOST.icon_id(), OPENCODE_CLI_HOST.icon_id());
    assert_eq!(OPENCODE_APP_HOST.config_format(), ConfigFormat::Json);
    assert_eq!(OPENCODE_CLI_HOST.config_format(), ConfigFormat::Json);
    assert_eq!(
        OPENCODE_APP_HOST.install_action_label(),
        OPENCODE_CLI_HOST.install_action_label()
    );
    assert!(!OPENCODE_CLI_HOST.can_open());
    assert_eq!(host_client(OPENCODE_APP_HOST.id()), ClientFamily::OpenAi);
    assert_eq!(host_client(OPENCODE_CLI_HOST.id()), ClientFamily::OpenAi);
}

#[test]
fn cli_only_installs_the_cli_host_and_not_the_app() {
    sandbox(true, |_| {
        assert_eq!(
            OPENCODE_CLI_HOST.probe(&probe_env()).app_installed,
            AppInstallState::Installed
        );
        assert_eq!(
            OPENCODE_APP_HOST.probe(&probe_env()).app_installed,
            AppInstallState::NotInstalled,
            "a binary on PATH is the CLI packaging; it never attests the desktop app"
        );
        assert!(
            !OPENCODE_APP_HOST.can_open(),
            "with no desktop bundle on this machine there is nothing to open"
        );
    });
}

#[test]
fn neither_packaging_present_is_a_conclusive_miss_for_both() {
    sandbox(false, |_| {
        assert_eq!(
            OPENCODE_CLI_HOST.probe(&probe_env()).app_installed,
            AppInstallState::NotInstalled
        );
        assert_eq!(
            OPENCODE_APP_HOST.probe(&probe_env()).app_installed,
            AppInstallState::NotInstalled
        );
    });
}

#[test]
fn processes_are_assigned_to_the_packaging_they_run_from() {
    let markers = &["@opencodedesktop"];
    let cli = classify_process(
        "opencode",
        "opencode",
        Some("/home/u/.opencode/bin/opencode"),
        markers,
    )
    .expect("the CLI binary is an opencode process");
    assert_eq!(cli.packaging, HostAppKind::CliTool);
    let bundle = classify_process(
        "opencode",
        "OpenCode",
        Some("/Applications/OpenCode.app/Contents/MacOS/OpenCode"),
        markers,
    )
    .expect("the bundle executable is an opencode process");
    assert_eq!(bundle.packaging, HostAppKind::DesktopApp);
    let electron = classify_process(
        "opencode",
        "opencode",
        Some("/mnt/c/Users/u/AppData/Local/Programs/@opencodedesktop/resources/opencode"),
        markers,
    )
    .expect("the bundled server binary is an opencode process");
    assert_eq!(
        electron.packaging,
        HostAppKind::DesktopApp,
        "a process under the Electron install folder is the app"
    );
}

#[test]
fn both_hosts_write_the_same_config_and_the_second_write_is_a_no_op() {
    sandbox(true, |user_config| {
        let first = OPENCODE_APP_HOST
            .generate_profile(&inputs_for(&OPENCODE_APP_HOST))
            .expect("app profile generated");
        OPENCODE_APP_HOST
            .install_profile(&first.path)
            .expect("app install");
        let after_app = std::fs::read(user_config).expect("opencode.json written by the app host");

        let second = OPENCODE_CLI_HOST
            .generate_profile(&inputs_for(&OPENCODE_CLI_HOST))
            .expect("cli profile generated");
        OPENCODE_CLI_HOST
            .install_profile(&second.path)
            .expect("cli install");
        let after_cli = std::fs::read(user_config).expect("opencode.json still present");
        assert_eq!(
            after_app, after_cli,
            "the CLI host writes the same bytes the app host wrote"
        );

        for host in [&OPENCODE_APP_HOST as &dyn HostApp, &OPENCODE_CLI_HOST] {
            let snapshot = host.probe(&probe_env());
            assert!(
                matches!(snapshot.profile_state, ProfileState::Installed),
                "{} reads the shared config as installed, got {:?}",
                host.id(),
                snapshot.profile_state
            );
        }
    });
}

#[test]
fn removal_through_either_host_strips_the_shared_config_once() {
    sandbox(true, |user_config| {
        let generated = OPENCODE_CLI_HOST
            .generate_profile(&inputs_for(&OPENCODE_CLI_HOST))
            .expect("cli profile generated");
        OPENCODE_CLI_HOST
            .install_profile(&generated.path)
            .expect("cli install");
        assert!(user_config.exists());

        let first = OPENCODE_APP_HOST.remove_profile().expect("app removal");
        assert!(
            matches!(
                first,
                systemprompt_bridge::integration::host_app::ProfileRemoval::Removed { .. }
            ),
            "{first:?}"
        );
        let second = OPENCODE_CLI_HOST.remove_profile().expect("cli removal");
        assert!(
            matches!(
                second,
                systemprompt_bridge::integration::host_app::ProfileRemoval::NothingToRemove
            ),
            "the sibling finds nothing of ours left: {second:?}"
        );
        assert!(
            matches!(
                OPENCODE_CLI_HOST.probe(&probe_env()).profile_state,
                ProfileState::Absent
            ),
            "after removal both packagings read the tree as absent"
        );
    });
}

#[test]
fn the_shared_config_is_kept_while_a_sibling_stays_enabled() {
    let both = [HostKind::OpenCode, HostKind::OpenCodeCli];
    assert_eq!(
        retained_for_sibling(HostKind::OpenCodeCli, &[HostKind::OpenCodeCli], &both),
        Some(HostKind::OpenCode)
    );
    assert_eq!(
        retained_for_sibling(HostKind::OpenCode, &[HostKind::OpenCode], &both),
        Some(HostKind::OpenCodeCli)
    );
    assert_eq!(retained_for_sibling(HostKind::OpenCode, &both, &both), None);
    assert_eq!(
        retained_for_sibling(
            HostKind::OpenCodeCli,
            &[HostKind::OpenCodeCli],
            &[HostKind::OpenCodeCli]
        ),
        None
    );
}
