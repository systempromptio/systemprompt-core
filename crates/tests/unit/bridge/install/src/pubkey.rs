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

// Why: `inferenceManifestPubkey` in Claude's hive is the key Claude Desktop
// 1.44121 logs as unrecognized on every launch; the pin is the bridge's and
// lives under the brand's own key.
#[test]
fn bridge_policy_values_carry_the_pin_under_its_own_key() {
    let values = systemprompt_bridge::install::bridge_policy_values(Some("BASE64-PUBKEY"));
    assert_eq!(
        values,
        vec![("manifestPubkey", "REG_SZ", "BASE64-PUBKEY".to_owned())]
    );
    assert!(systemprompt_bridge::install::bridge_policy_values(None).is_empty());
    let subkey = systemprompt_bridge::config::store::bridge_policy_subkey();
    assert!(subkey.starts_with(r"SOFTWARE\Policies\"), "{subkey}");
    assert_ne!(subkey, r"SOFTWARE\Policies\Claude");
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_never_put_the_pin_in_claudes_hive() {
    let values = systemprompt_bridge::install::windows_policy_values(None, None);
    let names: Vec<&str> = values.iter().map(|(n, _, _)| *n).collect();
    assert!(!names.contains(&"inferenceManifestPubkey"));
    assert!(!names.contains(&"manifestPubkey"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_includes_valid_org_uuid() {
    let values = systemprompt_bridge::install::windows_policy_values(
        Some("f8e4d915-f8ad-5304-ab0d-c1bf895df963"),
        None,
    );
    assert!(
        values
            .iter()
            .any(|(k, _, v)| *k == "deploymentOrganizationUuid"
                && v == "f8e4d915-f8ad-5304-ab0d-c1bf895df963")
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_omits_missing_or_invalid_org_uuid() {
    let none = systemprompt_bridge::install::windows_policy_values(None, None);
    assert!(
        !none
            .iter()
            .any(|(k, _, _)| *k == "deploymentOrganizationUuid")
    );
    let bad = systemprompt_bridge::install::windows_policy_values(Some("garbage"), None);
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
fn macos_claude_prefs_plist_never_carries_the_pin() {
    let plist = systemprompt_bridge::install::build_macos_prefs_plist(
        &mdm_inputs(),
        "https://gateway.example",
    );
    assert!(!plist.contains("ManifestPubkey"));
    assert!(!plist.contains("manifestPubkey"));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_bridge_prefs_plist_carries_the_pin() {
    let plist = systemprompt_bridge::install::build_macos_bridge_prefs_plist("BASE64-PUBKEY");
    assert!(plist.contains("<key>manifestPubkey</key><string>BASE64-PUBKEY</string>"));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_mobileconfig_carries_the_pin_as_its_own_payload() {
    let mc = systemprompt_bridge::install::build_macos_mobileconfig(
        &mdm_inputs(),
        "https://gateway.example",
        Some("BASE64-PUBKEY"),
    );
    assert!(mc.contains("<key>manifestPubkey</key><string>BASE64-PUBKEY</string>"));
    assert!(!mc.contains("inferenceManifestPubkey"));
    let domain = systemprompt_bridge::config::store::bridge_policy_domain();
    assert!(mc.contains(&format!("<key>PayloadType</key><string>{domain}</string>")));
    let without = systemprompt_bridge::install::build_macos_mobileconfig(
        &mdm_inputs(),
        "https://gateway.example",
        None,
    );
    assert!(!without.contains("manifestPubkey"));
    assert!(!without.contains(&domain));
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

#[cfg(target_os = "macos")]
static MDM_LOOPBACK: std::sync::LazyLock<systemprompt_bridge::proxy::LoopbackEndpoint> =
    std::sync::LazyLock::new(|| {
        systemprompt_bridge::proxy::LoopbackEndpoint::new(
            systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
            None,
        )
    });
#[cfg(target_os = "macos")]
static MDM_REGISTRY: std::sync::LazyLock<systemprompt_bridge::mcp_registry::McpRegistry> =
    std::sync::LazyLock::new(std::collections::HashMap::new);

#[cfg(target_os = "macos")]
fn mdm_inputs() -> systemprompt_bridge::install::MdmPayloadInputs<'static> {
    systemprompt_bridge::install::MdmPayloadInputs {
        loopback: &MDM_LOOPBACK,
        registry: &MDM_REGISTRY,
        egress_allowed_hosts: None,
    }
}

// Why: `disableNonessentialServices=true` blocks the renderer Cowork's MCP
// display extensions load from; the value is written as an explicit `false`
// so an older `true` is corrected by the next sync.
#[cfg(target_os = "windows")]
#[test]
fn windows_policy_values_keep_nonessential_services_enabled() {
    let values = systemprompt_bridge::install::windows_policy_values(None, None);
    let flag = values
        .iter()
        .find(|(name, _, _)| *name == "disableNonessentialServices")
        .expect("hardening flag present");
    assert_eq!(flag.2, "false");
    assert!(
        values
            .iter()
            .any(|(name, _, _)| *name == "allowedWorkspaceFolders")
    );
}
