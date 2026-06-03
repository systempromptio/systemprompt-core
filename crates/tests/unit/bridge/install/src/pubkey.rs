use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use systemprompt_bridge::config;
use systemprompt_bridge::sync::SyncError;

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-pubkey-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn pubkey_not_pinned_error_has_distinct_exit_code() {
    let err = SyncError::PubkeyNotPinned;
    let rendered = err.to_string();
    assert!(rendered.contains("not pinned"));
    assert!(rendered.contains("--allow-tofu"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_includes_pubkey_when_provided() {
    let values = systemprompt_bridge::install::windows_policy_values(
        "https://gateway.example",
        Some("BASE64-PUBKEY"),
        None,
    );
    let names: Vec<&str> = values.iter().map(|(n, _, _)| *n).collect();
    assert!(names.contains(&"inferenceManifestPubkey"));
    let pubkey_entry = values
        .iter()
        .find(|(n, _, _)| *n == "inferenceManifestPubkey")
        .unwrap();
    assert_eq!(pubkey_entry.1, "REG_SZ");
    assert_eq!(pubkey_entry.2, "BASE64-PUBKEY");
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_omits_pubkey_when_absent() {
    let values =
        systemprompt_bridge::install::windows_policy_values("https://gateway.example", None, None);
    let names: Vec<&str> = values.iter().map(|(n, _, _)| *n).collect();
    assert!(!names.contains(&"inferenceManifestPubkey"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_includes_valid_org_uuid() {
    let values = systemprompt_bridge::install::windows_policy_values(
        "https://gateway.example",
        None,
        Some("f8e4d915-f8ad-5304-ab0d-c1bf895df963"),
    );
    assert!(values.iter().any(|(k, _, v)| *k == "deploymentOrganizationUuid"
        && v == "f8e4d915-f8ad-5304-ab0d-c1bf895df963"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_omits_missing_or_invalid_org_uuid() {
    let none =
        systemprompt_bridge::install::windows_policy_values("https://gateway.example", None, None);
    assert!(
        !none
            .iter()
            .any(|(k, _, _)| *k == "deploymentOrganizationUuid")
    );
    let bad = systemprompt_bridge::install::windows_policy_values(
        "https://gateway.example",
        None,
        Some("garbage"),
    );
    assert!(
        !bad.iter()
            .any(|(k, _, _)| *k == "deploymentOrganizationUuid")
    );
}

#[test]
fn is_uuid_like_accepts_standard_hyphenated() {
    assert!(systemprompt_bridge::install::is_uuid_like(
        "f8e4d915-f8ad-5304-ab0d-c1bf895df963"
    ));
    assert!(systemprompt_bridge::install::is_uuid_like(
        "00000000-0000-4000-8000-000000000001"
    ));
}

#[test]
fn is_uuid_like_rejects_malformed() {
    assert!(!systemprompt_bridge::install::is_uuid_like(""));
    assert!(!systemprompt_bridge::install::is_uuid_like("not-a-uuid"));
    assert!(!systemprompt_bridge::install::is_uuid_like(
        "{f8e4d915-f8ad-5304-ab0d-c1bf895df963}"
    ));
    assert!(!systemprompt_bridge::install::is_uuid_like(
        "f8e4d915f8ad5304ab0dc1bf895df963"
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_prefs_plist_includes_pubkey_when_provided() {
    let plist = systemprompt_bridge::install::build_macos_prefs_plist(
        "https://gateway.example",
        Some("BASE64-PUBKEY"),
    );
    assert!(plist.contains("<key>inferenceManifestPubkey</key>"));
    assert!(plist.contains("<string>BASE64-PUBKEY</string>"));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_prefs_plist_omits_pubkey_when_absent() {
    let plist =
        systemprompt_bridge::install::build_macos_prefs_plist("https://gateway.example", None);
    assert!(!plist.contains("inferenceManifestPubkey"));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_mobileconfig_includes_pubkey_when_provided() {
    let mc = systemprompt_bridge::install::build_macos_mobileconfig(
        "https://gateway.example",
        Some("BASE64-PUBKEY"),
    );
    assert!(mc.contains("<key>inferenceManifestPubkey</key>"));
    assert!(mc.contains("<string>BASE64-PUBKEY</string>"));
}

#[test]
fn policy_pubkey_env_overrides_operator_set_value() {
    let _guard = env_lock();
    let dir = tempdir();
    let cfg_path = dir.join("systemprompt-bridge.toml");
    fs::write(&cfg_path, "[sync]\npinned_pubkey = \"OPERATOR-KEY-AAAA\"\n").unwrap();

    unsafe {
        std::env::set_var("SP_BRIDGE_CONFIG", &cfg_path);
        std::env::set_var("SP_BRIDGE_POLICY_PUBKEY", "POLICY-KEY-BBBB");
    }

    let pinned = config::pinned_pubkey();

    unsafe {
        std::env::remove_var("SP_BRIDGE_CONFIG");
        std::env::remove_var("SP_BRIDGE_POLICY_PUBKEY");
    }

    assert_eq!(pinned.as_ref().map(|p| p.as_str()), Some("POLICY-KEY-BBBB"));
}

#[test]
fn policy_pubkey_env_seeds_when_no_operator_value() {
    let _guard = env_lock();
    let dir = tempdir();
    let cfg_path = dir.join("systemprompt-bridge.toml");
    fs::write(&cfg_path, "").unwrap();

    unsafe {
        std::env::set_var("SP_BRIDGE_CONFIG", &cfg_path);
        std::env::set_var("SP_BRIDGE_POLICY_PUBKEY", "POLICY-KEY-CCCC");
    }

    let pinned = config::pinned_pubkey();

    unsafe {
        std::env::remove_var("SP_BRIDGE_CONFIG");
        std::env::remove_var("SP_BRIDGE_POLICY_PUBKEY");
    }

    assert_eq!(pinned.as_ref().map(|p| p.as_str()), Some("POLICY-KEY-CCCC"));
}

#[test]
fn no_pinned_pubkey_when_neither_operator_nor_policy_set() {
    let _guard = env_lock();
    let dir = tempdir();
    let cfg_path = dir.join("systemprompt-bridge.toml");
    fs::write(&cfg_path, "").unwrap();

    unsafe {
        std::env::set_var("SP_BRIDGE_CONFIG", &cfg_path);
        std::env::remove_var("SP_BRIDGE_POLICY_PUBKEY");
    }

    let pinned = config::pinned_pubkey();

    unsafe {
        std::env::remove_var("SP_BRIDGE_CONFIG");
    }

    assert!(pinned.is_none());
}

#[test]
fn policy_pubkey_helper_returns_env_value() {
    let _guard = env_lock();
    unsafe {
        std::env::set_var("SP_BRIDGE_POLICY_PUBKEY", "FROM-POLICY-DDDD");
    }
    let v = config::policy_pubkey();
    unsafe {
        std::env::remove_var("SP_BRIDGE_POLICY_PUBKEY");
    }
    assert_eq!(v.as_ref().map(|p| p.as_str()), Some("FROM-POLICY-DDDD"));
}
