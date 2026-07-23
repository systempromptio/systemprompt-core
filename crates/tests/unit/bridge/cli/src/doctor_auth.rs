use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::auth::{
    check_config_file, check_credential_source, check_loopback_secret, check_pinned_pubkey,
};
use systemprompt_bridge::config::Config;
use tempfile::TempDir;

fn with_config<R>(body: Option<&str>, f: impl FnOnce() -> R) -> R {
    let dir = TempDir::new().expect("config home");
    let brand_dir = dir.path().join("systemprompt");
    std::fs::create_dir_all(&brand_dir).expect("brand dir");
    if let Some(text) = body {
        std::fs::write(brand_dir.join("systemprompt-bridge.toml"), text).expect("config");
    }
    let vars: Vec<(&str, Option<String>)> = vec![
        ("XDG_CONFIG_HOME", Some(dir.path().display().to_string())),
        ("HOME", Some(dir.path().display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_PINNED_PUBKEY", None),
    ];
    let out = temp_env::with_vars(vars, f);
    drop(dir);
    out
}

#[test]
fn the_config_check_warns_when_no_config_has_been_written() {
    let check = with_config(None, check_config_file);
    assert_eq!(check.status, Status::Warn, "{}", check.detail);
    assert!(check.detail.contains("not present"), "{}", check.detail);
}

#[test]
fn the_config_check_passes_on_valid_toml_and_fails_on_a_parse_error() {
    let ok = with_config(
        Some("gateway_url = \"http://gw.invalid:7000\"\n"),
        check_config_file,
    );
    assert_eq!(ok.status, Status::Ok, "{}", ok.detail);
    assert!(ok.detail.contains("parses cleanly"), "{}", ok.detail);

    let bad = with_config(Some("gateway_url = [unterminated\n"), check_config_file);
    assert_eq!(bad.status, Status::Fail, "{}", bad.detail);
    assert!(bad.detail.contains("parse error"), "{}", bad.detail);
}

#[test]
fn the_credential_source_check_mirrors_the_configured_providers() {
    let none: Config = toml::from_str("").expect("config");
    let missing = with_config(None, || check_credential_source(&none));
    assert_eq!(missing.status, Status::Fail, "{}", missing.detail);

    let session: Config = toml::from_str("[session]\nenabled = true\n").expect("config");
    let present = with_config(None, || check_credential_source(&session));
    assert_eq!(present.status, Status::Ok, "{}", present.detail);
}

#[test]
fn the_loopback_secret_check_warns_when_unminted_and_passes_once_present() {
    let dir = TempDir::new().expect("config home");
    let brand_dir = dir.path().join("systemprompt");
    std::fs::create_dir_all(&brand_dir).expect("brand dir");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("XDG_CONFIG_HOME", Some(dir.path().display().to_string())),
        ("HOME", Some(dir.path().display().to_string())),
    ];

    let (before, after, empty) = temp_env::with_vars(vars, || {
        let before = check_loopback_secret();
        std::fs::write(brand_dir.join("bridge-loopback.key"), "a-secret-value").expect("secret");
        let after = check_loopback_secret();
        std::fs::write(brand_dir.join("bridge-loopback.key"), "   ").expect("blank secret");
        let empty = check_loopback_secret();
        (before, after, empty)
    });

    assert_eq!(before.status, Status::Warn, "{}", before.detail);
    assert!(before.detail.contains("re-apply"), "{}", before.detail);
    assert_eq!(after.status, Status::Ok, "{}", after.detail);
    assert_eq!(
        empty.status,
        Status::Warn,
        "a blank secret file reads as unminted: {}",
        empty.detail
    );
}

#[test]
fn the_pinned_pubkey_check_warns_until_a_key_is_pinned() {
    let unpinned = with_config(None, check_pinned_pubkey);
    assert_eq!(unpinned.status, Status::Warn, "{}", unpinned.detail);
    assert!(
        unpinned.detail.contains("allow-tofu"),
        "{}",
        unpinned.detail
    );

    let pinned = with_config(
        Some("[sync]\npinned_pubkey = \"dGVzdC1wdWJrZXk\"\n"),
        check_pinned_pubkey,
    );
    assert_eq!(pinned.status, Status::Ok, "{}", pinned.detail);
}
