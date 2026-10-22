//! Codex ships as a desktop app and a CLI over one configuration tree. The two
//! hosts must tell their own packaging apart and share the written config
//! without either overwriting the other.

use std::path::Path;

use systemprompt_bridge::integration::codex::{CODEX_APP_HOST, CODEX_CLI_HOST};
use systemprompt_bridge::integration::enrol::retained_for_sibling;
use systemprompt_bridge::integration::host_app::{
    AppInstallState, ConfigFormat, HostApp, ProfileState,
};
use systemprompt_bridge::sysproc::classify_process;
use systemprompt_models::bridge::host::{HostAppKind, HostKind};
use tempfile::TempDir;

use crate::support::{inputs_for, probe_env};

fn sandbox<R>(cli_on_path: bool, f: impl FnOnce(&Path) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin dir");
    if cli_on_path {
        std::fs::write(bin.join("codex"), "#!/bin/sh\n").expect("stub binary");
    }
    let managed = root.path().join("managed_config.toml");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        (
            "CODEX_HOME",
            Some(root.path().join("codex").display().to_string()),
        ),
        ("CODEX_SYSTEM_CONFIG", Some(managed.display().to_string())),
        ("PATH", Some(bin.display().to_string())),
        ("LOCALAPPDATA", None),
        ("APPDATA", None),
    ];
    let out = temp_env::with_vars(vars, || f(&managed));
    drop(root);
    out
}

#[test]
fn the_two_hosts_are_one_product_in_two_packagings() {
    assert_eq!(CODEX_APP_HOST.id(), HostKind::Codex);
    assert_eq!(CODEX_CLI_HOST.id(), HostKind::CodexCli);
    assert_eq!(CODEX_APP_HOST.kind(), HostAppKind::DesktopApp);
    assert_eq!(CODEX_CLI_HOST.kind(), HostAppKind::CliTool);
    assert_eq!(CODEX_APP_HOST.display_name(), "Codex");
    assert_eq!(CODEX_CLI_HOST.display_name(), "Codex CLI");
    assert_eq!(CODEX_APP_HOST.icon_id(), CODEX_CLI_HOST.icon_id());
    assert_eq!(CODEX_APP_HOST.config_format(), ConfigFormat::Toml);
    assert_eq!(CODEX_CLI_HOST.config_format(), ConfigFormat::Toml);
    assert_eq!(
        CODEX_APP_HOST.install_action_label(),
        CODEX_CLI_HOST.install_action_label(),
        "one config tree, one install step"
    );
    assert!(!CODEX_CLI_HOST.can_open(), "a CLI has no window to open");
    assert_ne!(CODEX_APP_HOST.download_url(), CODEX_CLI_HOST.download_url());
}

#[test]
fn cli_only_installs_the_cli_host_and_not_the_app() {
    sandbox(true, |_| {
        let cli = CODEX_CLI_HOST.probe(&probe_env());
        assert_eq!(cli.app_installed, AppInstallState::Installed);
        let app = CODEX_APP_HOST.probe(&probe_env());
        assert_eq!(
            app.app_installed,
            AppInstallState::NotInstalled,
            "a binary on PATH is the CLI packaging; it never attests the desktop app"
        );
    });
}

#[test]
fn neither_packaging_present_is_a_conclusive_miss_for_both() {
    sandbox(false, |_| {
        assert_eq!(
            CODEX_CLI_HOST.probe(&probe_env()).app_installed,
            AppInstallState::NotInstalled
        );
        assert_eq!(
            CODEX_APP_HOST.probe(&probe_env()).app_installed,
            AppInstallState::NotInstalled
        );
    });
}

#[test]
fn processes_are_assigned_to_the_packaging_they_run_from() {
    let markers = &["\\programs\\codex\\"];
    let cli = classify_process("codex", "codex", Some("/usr/local/bin/codex"), markers)
        .expect("the CLI binary is a codex process");
    assert_eq!(cli.packaging, HostAppKind::CliTool);
    let bundle = classify_process(
        "codex",
        "Codex",
        Some("/Applications/Codex.app/Contents/MacOS/Codex"),
        markers,
    )
    .expect("the bundle executable is a codex process");
    assert_eq!(bundle.packaging, HostAppKind::DesktopApp);
    assert!(
        classify_process("codex", "node", Some("/usr/bin/node"), markers).is_none(),
        "an unrelated process is not a hit"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn both_hosts_write_the_same_config_and_the_second_write_is_a_no_op() {
    sandbox(true, |managed| {
        let first = CODEX_APP_HOST
            .generate_profile(&inputs_for(&CODEX_APP_HOST))
            .expect("app profile generated");
        CODEX_APP_HOST
            .install_profile(&first.path)
            .expect("app install");
        let after_app = std::fs::read(managed).expect("managed config written by the app host");

        let second = CODEX_CLI_HOST
            .generate_profile(&inputs_for(&CODEX_CLI_HOST))
            .expect("cli profile generated");
        CODEX_CLI_HOST
            .install_profile(&second.path)
            .expect("cli install");
        let after_cli = std::fs::read(managed).expect("managed config still present");
        assert_eq!(
            after_app, after_cli,
            "the CLI host writes the same bytes the app host wrote"
        );

        for host in [&CODEX_APP_HOST as &dyn HostApp, &CODEX_CLI_HOST] {
            let snapshot = host.probe(&probe_env());
            assert!(
                matches!(snapshot.profile_state, ProfileState::Installed),
                "{} reads the shared config as installed, got {:?}",
                host.id(),
                snapshot.profile_state
            );
        }
        let text = String::from_utf8(after_cli).expect("toml is utf-8");
        assert!(
            text.contains("--host\", \"codex-cli\"") || text.contains("\"codex-cli\""),
            "one credential helper scope for both packagings: {text}"
        );
    });
}

#[test]
fn the_shared_config_is_kept_while_a_sibling_stays_enabled() {
    let both = [HostKind::Codex, HostKind::CodexCli];
    assert_eq!(
        retained_for_sibling(HostKind::Codex, &[HostKind::Codex], &both),
        Some(HostKind::CodexCli),
        "removing the app while the CLI is enabled keeps the tree"
    );
    assert_eq!(
        retained_for_sibling(HostKind::CodexCli, &[HostKind::CodexCli], &both),
        Some(HostKind::Codex)
    );
    assert_eq!(
        retained_for_sibling(HostKind::Codex, &both, &both),
        None,
        "removing both packagings together strips the tree"
    );
    assert_eq!(
        retained_for_sibling(HostKind::Codex, &[HostKind::Codex], &[HostKind::Codex]),
        None,
        "a sibling that is not enabled holds nothing back"
    );
    assert_eq!(
        retained_for_sibling(HostKind::Hermes, &[HostKind::Hermes], &HostKind::ALL),
        None,
        "a host with its own config tree is never retained"
    );
}
