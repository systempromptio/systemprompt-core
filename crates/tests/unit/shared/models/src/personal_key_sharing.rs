//! One personal subscription key covers every provider entry that names it in
//! `credential_from`: OpenCode Go serves Chat Completions and Anthropic
//! Messages from two entries, and a member stores the key once.

use std::collections::BTreeSet;

use systemprompt_manifest::bridge_profile::provider_health_for_user;
use systemprompt_manifest::services::{ProviderEntry, ProviderRegistry};

const GO_CHAT: &str = "name: opencode-go
wire: openai-chat
surface: openai
endpoint: https://go.example.test/v1
credential: user
models:
  - id: go-kimi-k3
";

const GO_ANTHROPIC: &str = "name: opencode-go-anthropic
wire: anthropic
surface: openai
endpoint: https://go.example.test/anthropic/v1
credential: user
credential_from: opencode-go
models:
  - id: go-minimax-m3
";

fn entry(yaml: &str) -> ProviderEntry {
    serde_yaml::from_str(yaml).expect("provider entry")
}

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![entry(GO_CHAT), entry(GO_ANTHROPIC)],
    }
}

fn keys(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

#[test]
fn a_shared_entry_reads_the_key_of_the_entry_it_names() {
    let anthropic = entry(GO_ANTHROPIC);
    assert_eq!(anthropic.credential_owner(), "opencode-go");
    assert!(!anthropic.takes_own_personal_key());

    let chat = entry(GO_CHAT);
    assert_eq!(chat.credential_owner(), "opencode-go");
    assert!(chat.takes_own_personal_key());
}

#[test]
fn one_stored_key_configures_both_entries() {
    let health = provider_health_for_user(&registry(), &|_| false, &keys(&["opencode-go"]));
    assert!(health.iter().all(|p| p.configured), "{health:?}");
}

#[test]
fn without_the_key_neither_entry_is_configured_and_only_the_owner_asks_for_it() {
    let health = provider_health_for_user(&registry(), &|_| false, &keys(&[]));
    assert!(health.iter().all(|p| !p.configured), "{health:?}");
    let asking: Vec<_> = health
        .iter()
        .filter(|p| p.personal_key)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(asking, ["opencode-go"]);
}

const OTHER_PERSONAL: &str = "name: other-sub
wire: anthropic
surface: openai
endpoint: https://other.example.test/v1
credential: user
models:
  - id: other-model
";

const PLATFORM_A: &str = "name: platform-a
wire: anthropic
surface: anthropic
endpoint: https://a.example.test/v1
api_key_secret: a
models:
  - id: a-model
";

const PLATFORM_B: &str = "name: platform-b
wire: anthropic
surface: anthropic
endpoint: https://b.example.test/v1
api_key_secret: b
models:
  - id: b-model
";

#[test]
fn entries_sharing_one_personal_key_bill_the_same_credential() {
    assert!(entry(GO_CHAT).bills_same_credential(&entry(GO_ANTHROPIC)));
    assert!(entry(GO_ANTHROPIC).bills_same_credential(&entry(GO_CHAT)));
}

#[test]
fn two_different_personal_keys_never_bill_the_same_credential() {
    assert!(!entry(GO_CHAT).bills_same_credential(&entry(OTHER_PERSONAL)));
    assert!(!entry(OTHER_PERSONAL).bills_same_credential(&entry(GO_ANTHROPIC)));
}

#[test]
fn personal_and_platform_entries_never_bill_the_same_credential() {
    assert!(!entry(GO_CHAT).bills_same_credential(&entry(PLATFORM_A)));
    assert!(!entry(PLATFORM_A).bills_same_credential(&entry(GO_CHAT)));
}

#[test]
fn platform_entries_bill_the_deployment_and_may_fail_over_to_each_other() {
    assert!(entry(PLATFORM_A).bills_same_credential(&entry(PLATFORM_B)));
}
