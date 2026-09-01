//! Cowork egress allowlist resolution.
//!
//! The allowlist is unrestricted by default. It used to be hard-pinned to
//! `127.0.0.1`, which left agents on a stock install with no internet access at
//! all; loopback-only is now an explicit opt-in for regulated deployments.

use std::sync::{Mutex, MutexGuard, OnceLock};

use systemprompt_bridge::install::cowork_egress_allowed_hosts;

const ENV: &str = "SP_BRIDGE_EGRESS_ALLOWED_HOSTS";

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn with_env(value: Option<&str>, f: impl FnOnce() -> Option<Vec<String>>) -> Option<Vec<String>> {
    let _guard = env_lock();
    unsafe {
        match value {
            Some(v) => std::env::set_var(ENV, v),
            None => std::env::remove_var(ENV),
        }
    }
    let out = f();
    unsafe {
        std::env::remove_var(ENV);
    }
    out
}

#[test]
fn unset_means_unrestricted() {
    assert_eq!(with_env(None, || cowork_egress_allowed_hosts(None)), None);
}

#[test]
fn loopback_alias_expands_to_localhost() {
    assert_eq!(
        with_env(Some("loopback"), || cowork_egress_allowed_hosts(None)),
        Some(vec!["127.0.0.1".to_owned()])
    );
}

#[test]
fn alias_is_case_insensitive() {
    assert_eq!(
        with_env(Some("LoopBack"), || cowork_egress_allowed_hosts(None)),
        Some(vec!["127.0.0.1".to_owned()])
    );
}

#[test]
fn explicit_hosts_are_split_and_trimmed() {
    assert_eq!(
        with_env(Some(" github.com , loopback ,api.example.com "), || {
            cowork_egress_allowed_hosts(None)
        }),
        Some(vec![
            "github.com".to_owned(),
            "127.0.0.1".to_owned(),
            "api.example.com".to_owned(),
        ])
    );
}

/// An empty value must not render as an empty allowlist — that would block
/// every host, the opposite of what clearing the variable reads as.
#[test]
fn empty_value_means_unrestricted() {
    assert_eq!(
        with_env(Some(""), || cowork_egress_allowed_hosts(None)),
        None
    );
    assert_eq!(
        with_env(Some("  , ,"), || cowork_egress_allowed_hosts(None)),
        None
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_omits_egress_key_by_default() {
    let _guard = env_lock();
    unsafe {
        std::env::remove_var(ENV);
    }
    let values =
        systemprompt_bridge::install::windows_policy_values("https://gateway.example", None, None);
    assert!(
        !values
            .iter()
            .any(|(k, _, _)| *k == "coworkEgressAllowedHosts"),
        "a stock install must not restrict Cowork egress"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_policy_writes_json_array_when_opted_in() {
    let _guard = env_lock();
    unsafe {
        std::env::set_var(ENV, "loopback");
    }
    let values =
        systemprompt_bridge::install::windows_policy_values("https://gateway.example", None, None);
    unsafe {
        std::env::remove_var(ENV);
    }
    let entry = values
        .iter()
        .find(|(k, _, _)| *k == "coworkEgressAllowedHosts")
        .expect("opt-in must write the policy value");
    assert_eq!(entry.1, "REG_SZ");
    assert_eq!(entry.2, r#"["127.0.0.1"]"#);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_payloads_omit_egress_key_by_default() {
    let _guard = env_lock();
    unsafe {
        std::env::remove_var(ENV);
    }
    let plist =
        systemprompt_bridge::install::build_macos_prefs_plist("https://gateway.example", None);
    let mc =
        systemprompt_bridge::install::build_macos_mobileconfig("https://gateway.example", None);
    assert!(!plist.contains("coworkEgressAllowedHosts"), "{plist}");
    assert!(!mc.contains("coworkEgressAllowedHosts"), "{mc}");
    for rendered in [&plist, &mc] {
        assert!(
            !rendered.contains("{egress_block}"),
            "the placeholder must be substituted, not left literal: {rendered}"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_payloads_render_array_when_opted_in() {
    let _guard = env_lock();
    unsafe {
        std::env::set_var(ENV, "loopback");
    }
    let plist =
        systemprompt_bridge::install::build_macos_prefs_plist("https://gateway.example", None);
    let mc =
        systemprompt_bridge::install::build_macos_mobileconfig("https://gateway.example", None);
    unsafe {
        std::env::remove_var(ENV);
    }
    for rendered in [&plist, &mc] {
        assert!(
            rendered.contains("<key>coworkEgressAllowedHosts</key>"),
            "{rendered}"
        );
        assert!(
            rendered.contains("<string>127.0.0.1</string>"),
            "{rendered}"
        );
    }
}
