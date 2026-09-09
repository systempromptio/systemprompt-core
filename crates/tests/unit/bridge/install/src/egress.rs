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

// An empty value must not render as an empty allowlist — that would block
// every host, the opposite of what clearing the variable reads as.
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

#[cfg(target_os = "macos")]
#[test]
fn macos_payloads_omit_egress_key_by_default() {
    let _guard = env_lock();
    unsafe {
        std::env::remove_var(ENV);
    }
    let hosts = systemprompt_bridge::install::cowork_egress_allowed_hosts(None);
    let inputs = mdm_inputs(hosts.as_deref());
    let plist =
        systemprompt_bridge::install::build_macos_prefs_plist(&inputs, "https://gateway.example")
            .expect("prefs plist");
    let mc = systemprompt_bridge::install::build_macos_mobileconfig(
        &inputs,
        "https://gateway.example",
        None,
    )
    .expect("mobileconfig");
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
    let hosts = systemprompt_bridge::install::cowork_egress_allowed_hosts(None);
    let inputs = mdm_inputs(hosts.as_deref());
    let plist =
        systemprompt_bridge::install::build_macos_prefs_plist(&inputs, "https://gateway.example")
            .expect("prefs plist");
    let mc = systemprompt_bridge::install::build_macos_mobileconfig(
        &inputs,
        "https://gateway.example",
        None,
    )
    .expect("mobileconfig");
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

#[cfg(target_os = "macos")]
static MDM_LOOPBACK: std::sync::LazyLock<systemprompt_bridge::proxy::LoopbackEndpoint> =
    std::sync::LazyLock::new(|| {
        systemprompt_bridge::proxy::LoopbackEndpoint::new(
            systemprompt_bridge::proxy::DEFAULT_PROXY_PORT,
            Some(systemprompt_bridge::ids::LoopbackSecret::new("mdm-test-secret")),
        )
    });
#[cfg(target_os = "macos")]
static MDM_REGISTRY: std::sync::LazyLock<systemprompt_bridge::mcp_registry::McpRegistry> =
    std::sync::LazyLock::new(std::collections::HashMap::new);

#[cfg(target_os = "macos")]
static MDM_POLICY_STORE: std::sync::LazyLock<systemprompt_bridge::config::store::PolicyStore> =
    std::sync::LazyLock::new(|| {
        systemprompt_bridge::config::store::PolicyStore::new(
            systemprompt_bridge::config::store::managed_policy_store(),
        )
    });

#[cfg(target_os = "macos")]
fn mdm_inputs(
    egress_allowed_hosts: Option<&[String]>,
) -> systemprompt_bridge::install::MdmPayloadInputs<'_> {
    systemprompt_bridge::install::MdmPayloadInputs {
        policy_store: &MDM_POLICY_STORE,
        loopback: &MDM_LOOPBACK,
        registry: &MDM_REGISTRY,
        egress_allowed_hosts,
    }
}
