//! The doctor check that names the failure mode where every other check is
//! green but `claude plugin list` is empty.

use std::path::Path;

use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::marketplace::check_marketplace;
use tempfile::TempDir;

fn with_home<R>(prepare: impl FnOnce(&Path), f: impl FnOnce() -> R) -> R {
    let dir = TempDir::new().expect("home");
    prepare(dir.path());
    let out = temp_env::with_vars(
        [
            ("HOME", Some(dir.path().display().to_string())),
            ("XDG_CONFIG_HOME", Some(dir.path().display().to_string())),
            ("SUDO_USER", None),
            ("PATH", Some(String::new())),
        ],
        f,
    );
    drop(dir);
    out
}

fn write_manifest(home: &Path, body: &str) {
    let dir = home
        .join(".claude")
        .join("plugins")
        .join("marketplaces")
        .join("org-provisioned")
        .join(".claude-plugin");
    std::fs::create_dir_all(&dir).expect("marketplace dir");
    std::fs::write(dir.join("marketplace.json"), body).expect("manifest");
}

#[test]
fn with_no_claude_code_cli_installed_the_check_warns_and_says_how_to_install_it() {
    let check = with_home(|_| {}, check_marketplace);
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("@anthropic-ai/claude-code"),
        "{}",
        check.detail
    );
}

#[test]
fn with_the_cli_present_but_no_manifest_yet_the_check_warns_and_points_at_sync() {
    let check = with_home(
        |home| {
            std::fs::create_dir_all(home.join(".claude")).expect(".claude");
        },
        check_marketplace,
    );
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("marketplace.json"),
        "{}",
        check.detail
    );
    assert!(check.detail.contains("sync"), "{}", check.detail);
}

#[test]
fn a_manifest_that_is_not_valid_json_is_a_failure_rather_than_a_warning() {
    let check = with_home(|home| write_manifest(home, "{not json"), check_marketplace);
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains("not valid JSON"), "{}", check.detail);
}

#[test]
fn a_manifest_listing_no_plugins_warns_rather_than_reporting_a_healthy_marketplace() {
    let check = with_home(
        |home| write_manifest(home, r#"{"plugins": []}"#),
        check_marketplace,
    );
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("lists no plugins"),
        "{}",
        check.detail
    );
}

#[test]
fn a_manifest_with_no_plugins_key_at_all_is_treated_the_same_as_an_empty_one() {
    let check = with_home(
        |home| write_manifest(home, r#"{"name": "org-provisioned"}"#),
        check_marketplace,
    );
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("lists no plugins"),
        "{}",
        check.detail
    );
}

#[test]
fn a_populated_manifest_passes_and_reports_how_many_plugins_are_registered() {
    let check = with_home(
        |home| write_manifest(home, r#"{"plugins": [{"name": "a"}, {"name": "b"}]}"#),
        check_marketplace,
    );
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(check.detail.contains("org-provisioned"), "{}", check.detail);
    assert!(
        check.detail.contains("2 plugin(s)"),
        "the count comes from the manifest, got {}",
        check.detail
    );
}

#[test]
fn a_plugins_key_that_is_not_an_array_is_treated_as_no_plugins_rather_than_crashing() {
    let check = with_home(
        |home| write_manifest(home, r#"{"plugins": "all of them"}"#),
        check_marketplace,
    );
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(
        check.detail.contains("lists no plugins"),
        "{}",
        check.detail
    );
}
