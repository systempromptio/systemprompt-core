use std::path::PathBuf;

use systemprompt_bridge::auth::setup::{CleanReport, PathLayout};
use systemprompt_bridge::install::{ManagedProfileOutcome, UninstallSummary};
use systemprompt_bridge::integration::uninstall::{PurgeReport, remove_proxy_state};
use systemprompt_bridge::proxy::{DEFAULT_PROXY_PORT, identity, portfile, secret};
use tempfile::TempDir;

fn in_sandbox<R>(config: &TempDir, f: impl FnOnce() -> R) -> R {
    temp_env::with_var("XDG_CONFIG_HOME", Some(config.path().as_os_str()), f)
}

fn seed(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, b"stale").expect("seed");
}

fn report(
    warnings: Vec<String>,
    summary: UninstallSummary,
    foreign: Option<String>,
) -> PurgeReport {
    PurgeReport {
        uninstall: summary,
        clean: CleanReport {
            paths: PathLayout {
                config_dir: PathBuf::from("/tmp/none"),
                config_file: PathBuf::from("/tmp/none/bridge.toml"),
                pat_file: PathBuf::from("/tmp/none/bridge.pat"),
            },
            pat_removed: true,
            config_removed: true,
            oauth_creds_removed: true,
        },
        warnings,
        foreign_proxy: foreign,
        proxy_state_removed: Vec::new(),
    }
}

#[test]
fn purging_removes_the_loopback_secret_install_id_and_portfile() {
    let config = TempDir::new().expect("config tempdir");
    let removed = in_sandbox(&config, || {
        for path in [
            secret::secret_path(),
            identity::install_id_path(),
            portfile::portfile_path(),
        ]
        .into_iter()
        .flatten()
        {
            seed(&path);
        }
        remove_proxy_state().expect("the sandboxed state files are removable")
    });

    assert_eq!(
        removed.len(),
        3,
        "every seeded state file is reported as removed: {removed:?}"
    );
    for path in &removed {
        assert!(!path.exists(), "{} survived the purge", path.display());
    }
}

#[test]
fn purging_a_device_with_no_proxy_state_removes_nothing_and_succeeds() {
    let config = TempDir::new().expect("config tempdir");
    let removed = in_sandbox(&config, || {
        remove_proxy_state().expect("absent state is not an error")
    });
    assert!(
        removed.is_empty(),
        "nothing was there to remove: {removed:?}"
    );
}

#[test]
fn leftovers_name_a_managed_policy_that_could_not_be_removed() {
    let summary = UninstallSummary::builder()
        .managed_profile(ManagedProfileOutcome::RemoveFailed(
            "access is denied".to_owned(),
        ))
        .build();

    let leftovers = report(
        vec!["Cowork enable-key cleanup failed: eacces".to_owned()],
        summary,
        None,
    )
    .leftovers();

    assert_eq!(leftovers.len(), 2, "{leftovers:?}");
    assert!(
        leftovers[0].contains("Cowork enable-key"),
        "the collected warnings come first: {leftovers:?}"
    );
    assert!(
        leftovers[1].contains("managed Claude policy") && leftovers[1].contains("access is denied"),
        "the policy failure is reported with its cause: {leftovers:?}"
    );
}

#[test]
fn leftovers_explain_that_another_accounts_bridge_still_holds_the_port() {
    let summary = UninstallSummary::builder().build();

    let leftovers = report(
        Vec::new(),
        summary,
        Some("/home/other/.config/systemprompt".to_owned()),
    )
    .leftovers();

    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert!(
        leftovers[0].contains("/home/other/.config/systemprompt"),
        "the other account's config dir is named: {leftovers:?}"
    );
    assert!(
        leftovers[0].contains(&DEFAULT_PROXY_PORT.to_string()),
        "the port it keeps is named: {leftovers:?}"
    );
}

#[test]
fn a_clean_purge_has_no_leftovers() {
    let summary = UninstallSummary::builder()
        .managed_profile(ManagedProfileOutcome::NotInstalled("linux"))
        .build();

    assert!(report(Vec::new(), summary, None).leftovers().is_empty());
}
