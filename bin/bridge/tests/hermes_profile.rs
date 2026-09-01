#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "a test asserts by failing loudly; there is no caller to return an error to"
)]

//! The Hermes host profile is a contract with a program we do not control, and
//! every part of it failed silently before: a probe sees well-formed keys and
//! reports healthy while Hermes ignores every one of them. These tests pin the
//! four keys that make inference route, and prove the merge is non-destructive.
//!
//! Verified against Hermes Agent 0.21.0.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use systemprompt_bridge::integration::HostApp;
use systemprompt_bridge::integration::hermes::{
    HERMES_HOST, contract, install_profile_into, remove_profile_from,
};
use systemprompt_bridge::integration::host_app::{ProfileGenInputs, ProfileRemoval};

const GATEWAY: &str = "http://127.0.0.1:48217";
const MODEL: &str = "claude-haiku-4-5";
const SECRET: &str = "loopback-secret-value";

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: GATEWAY.to_owned(),
        api_key: SECRET.to_owned(),
        models: vec![MODEL.to_owned()],
        organization_uuid: None,
        headers: BTreeMap::new(),
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hermes-profile-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn read_yaml(path: &Path) -> serde_yaml::Value {
    serde_yaml::from_str(&std::fs::read_to_string(path).expect("read yaml")).expect("parse yaml")
}

fn at<'a>(root: &'a serde_yaml::Value, path: &str) -> &'a serde_yaml::Value {
    let mut cur = root;
    for segment in path.split('.') {
        cur = cur
            .as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String(segment.to_owned())))
            .unwrap_or_else(|| panic!("missing key {path}"));
    }
    cur
}

fn dotted(root: &serde_yaml::Value, path: &str) -> String {
    let mut cur = root;
    for segment in path.split('.') {
        cur = cur
            .as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String(segment.to_owned())))
            .unwrap_or_else(|| panic!("missing key {path}"));
    }
    cur.as_str()
        .unwrap_or_else(|| panic!("{path} is not a string"))
        .to_owned()
}

fn install_into(home: &Path) {
    let generated = HERMES_HOST.generate_profile(&inputs()).expect("generate");
    install_profile_into(&generated.path, home).expect("install");
    std::fs::remove_file(&generated.path).ok();
}

/// Each of these four was wrong once, and each failure was silent.
///
/// `model.provider` selects the provider before `base_url` is consulted at all,
/// so an endpoint with the default `auto` is never reached. `api_mode` has a
/// closed vocabulary that does not contain "openai". `key_env` is the only way
/// a 127.0.0.1 endpoint resolves a credential, because Hermes host-gates its
/// bare `OPENAI_API_KEY` fallback to openai.com. And the model has to be
/// `model.default`, which wins over `model.model` whenever both are set —
/// which is always, because Hermes ships a `default`.
#[test]
fn profile_writes_the_four_keys_hermes_actually_reads() {
    let home = scratch("contract");
    install_into(&home);

    let cfg = read_yaml(&home.join("config.yaml"));
    assert_eq!(
        dotted(&cfg, contract::MODEL_PROVIDER),
        contract::PROVIDER_ENTRY
    );
    assert_eq!(
        dotted(&cfg, contract::PROVIDER_BASE_URL),
        format!("{GATEWAY}/v1")
    );
    assert_eq!(
        dotted(&cfg, contract::PROVIDER_API_MODE),
        "chat_completions"
    );
    assert_eq!(
        dotted(&cfg, contract::PROVIDER_KEY_ENV),
        contract::ENV_API_KEY
    );
    assert_eq!(dotted(&cfg, contract::MODEL_NAME), MODEL);

    // The secret belongs in .env at 0600, never in config.yaml.
    let env = std::fs::read_to_string(home.join(".env")).expect("read .env");
    assert!(env.contains(&format!("{}={SECRET}", contract::ENV_API_KEY)));
    assert!(
        !std::fs::read_to_string(home.join("config.yaml"))
            .expect("read config")
            .contains(SECRET)
    );

    std::fs::remove_dir_all(&home).ok();
}

/// Hermes' shipped `config.yaml` is a large file the user edits in place, and
/// `providers:` is shared ground — their own entries live in the same table as
/// ours. Installing must not disturb either, and removing must take back only
/// what we put there.
#[test]
fn install_and_remove_leave_every_foreign_key_untouched() {
    let home = scratch("merge");
    let config = home.join("config.yaml");
    std::fs::write(
        &config,
        "model:\n  default: anthropic/claude-opus-4.6\n  provider: auto\n  \
         context_length: 131072\nproviders:\n  mine:\n    base_url: \
         https://example.invalid/v1\nterminal:\n  backend: local\n",
    )
    .expect("seed config");
    std::fs::write(home.join(".env"), "BROWSER_SESSION_TIMEOUT=300\n").expect("seed env");

    install_into(&home);

    let after = read_yaml(&config);
    assert_eq!(at(&after, "model.context_length").as_u64(), Some(131_072));
    assert_eq!(
        dotted(&after, "providers.mine.base_url"),
        "https://example.invalid/v1"
    );
    assert_eq!(dotted(&after, "terminal.backend"), "local");

    let removed = remove_profile_from(&home).expect("remove");
    assert!(matches!(removed, ProfileRemoval::Removed { .. }));

    let restored = read_yaml(&config);
    assert_eq!(
        at(&restored, "model.context_length").as_u64(),
        Some(131_072)
    );
    assert_eq!(
        dotted(&restored, "providers.mine.base_url"),
        "https://example.invalid/v1"
    );
    assert_eq!(dotted(&restored, "terminal.backend"), "local");
    assert!(
        restored
            .get(serde_yaml::Value::String("providers".to_owned()))
            .and_then(serde_yaml::Value::as_mapping)
            .is_some_and(|m| !m.contains_key(serde_yaml::Value::String(
                contract::PROVIDER_ENTRY.to_owned()
            )))
    );

    // The user's other secrets survive; ours is gone.
    let env = std::fs::read_to_string(home.join(".env")).expect("read .env");
    assert!(env.contains("BROWSER_SESSION_TIMEOUT=300"));
    assert!(!env.contains(contract::ENV_API_KEY));

    std::fs::remove_dir_all(&home).ok();
}

/// Re-applying is the documented remedy for a rotated loopback secret, so it
/// has to converge rather than accumulate a second endpoint.
#[test]
fn reinstall_replaces_the_managed_entry_rather_than_duplicating_it() {
    let home = scratch("reinstall");
    install_into(&home);
    install_into(&home);

    let cfg = read_yaml(&home.join("config.yaml"));
    let providers = cfg
        .get(serde_yaml::Value::String("providers".to_owned()))
        .and_then(serde_yaml::Value::as_mapping)
        .expect("providers table");
    assert_eq!(providers.len(), 1);
    assert_eq!(
        dotted(&cfg, contract::PROVIDER_BASE_URL),
        format!("{GATEWAY}/v1")
    );

    std::fs::remove_dir_all(&home).ok();
}
