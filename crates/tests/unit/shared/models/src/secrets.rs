use std::collections::HashMap;
use systemprompt_manifest::child_env::{ChildEnvError, ChildGrants};
use systemprompt_manifest::secrets::Secrets;

fn full_secrets() -> Secrets {
    let mut custom = HashMap::new();
    custom.insert("STRIPE_KEY".to_owned(), "sk_test".to_owned());
    custom.insert("intercom".to_owned(), "ic_value".to_owned());
    Secrets {
        oauth_at_rest_pepper: "p".repeat(32),
        manifest_signing_secret_seed: Some("seed".to_owned()),
        signing_key_pem: None,
        signing_key_staged_pem: Some("staged".to_owned()),
        signing_key_previous_keys: Some("[]".to_owned()),
        database_url: "postgres://primary".to_owned(),
        database_write_url: Some("postgres://write".to_owned()),
        database_session_url: None,
        external_database_url: Some("postgres://external".to_owned()),
        internal_database_url: Some("postgres://internal".to_owned()),
        gemini: Some("g".to_owned()),
        anthropic: Some("a".to_owned()),
        openai: Some("o".to_owned()),
        github: Some("gh".to_owned()),
        moonshot: Some("m".to_owned()),
        qwen: Some("q".to_owned()),
        custom,
    }
}

fn minimal_secrets() -> Secrets {
    Secrets {
        oauth_at_rest_pepper: "p".repeat(32),
        manifest_signing_secret_seed: None,
        signing_key_pem: None,
        signing_key_staged_pem: None,
        signing_key_previous_keys: None,
        database_url: "postgres://primary".to_owned(),
        database_write_url: None,
        database_session_url: None,
        external_database_url: None,
        internal_database_url: None,
        gemini: None,
        anthropic: None,
        openai: None,
        github: None,
        moonshot: None,
        qwen: None,
        custom: HashMap::new(),
    }
}

const API_HELD: &[&str] = &[
    "OAUTH_AT_REST_PEPPER",
    "MANIFEST_SIGNING_SECRET_SEED",
    "SIGNING_KEY_PEM",
    "SIGNING_KEY_STAGED_PEM",
    "SIGNING_KEY_PREVIOUS_KEYS",
    "ENCRYPTION_MASTER_KEY",
    "LICENCE_SIGNING_SECRET",
];

fn env_of(secrets: &Secrets, explicit: &[&str], derived: &[&str]) -> HashMap<String, String> {
    let grants = ChildGrants {
        explicit: explicit.iter().map(|g| g.parse().expect("grant")).collect(),
        derived: derived.iter().map(|g| g.parse().expect("grant")).collect(),
    };
    secrets
        .child_env(&grants)
        .expect("child env")
        .into_iter()
        .collect()
}

fn with_master_key(mut secrets: Secrets) -> Secrets {
    secrets
        .custom
        .insert("encryption_master_key".to_owned(), "mk".to_owned());
    secrets
        .custom
        .insert("LICENCE_SIGNING_SECRET".to_owned(), "lic".to_owned());
    secrets
}

#[test]
fn a_child_without_grants_gets_no_provider_key_and_no_custom_secret() {
    let env = env_of(&with_master_key(full_secrets()), &[], &[]);
    for key in [
        "GEMINI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "MOONSHOT_API_KEY",
        "QWEN_API_KEY",
        "STRIPE_KEY",
        "INTERCOM",
        "intercom",
    ] {
        assert!(!env.contains_key(key), "{key} reached an ungranted child");
    }
    assert!(!env.contains_key("SYSTEMPROMPT_CUSTOM_SECRETS"));
    assert!(!env.contains_key("ENCRYPTION_MASTER_KEY"));
}

#[test]
fn a_child_gets_none_of_the_api_held_secrets() {
    let mut secrets = with_master_key(full_secrets());
    secrets.signing_key_pem = Some("pem".to_owned());
    secrets.signing_key_staged_pem = Some("staged".to_owned());
    secrets.signing_key_previous_keys = Some("[]".to_owned());
    let env = env_of(&secrets, &["anthropic", "custom:stripe_key"], &["openai"]);
    for key in API_HELD {
        assert!(!env.contains_key(*key), "{key} reached a child");
    }
    let held = [
        "p".repeat(32),
        "seed".to_owned(),
        "pem".to_owned(),
        "staged".to_owned(),
        "mk".to_owned(),
        "lic".to_owned(),
    ];
    assert!(
        env.values().all(|value| !held.contains(value)),
        "an API-held value reached a host child under another name"
    );
}

#[test]
fn a_host_child_gets_the_master_key_only_through_its_grant() {
    let env = env_of(
        &with_master_key(full_secrets()),
        &["encryption_master_key"],
        &[],
    );
    assert_eq!(
        env.get("ENCRYPTION_MASTER_KEY").map(String::as_str),
        Some("mk")
    );
    assert_eq!(
        env.get("SYSTEMPROMPT_CUSTOM_SECRETS").map(String::as_str),
        Some("ENCRYPTION_MASTER_KEY")
    );
    assert!(!env.contains_key("OAUTH_AT_REST_PEPPER"));
    assert!(!env.contains_key("LICENCE_SIGNING_SECRET"));
}

#[test]
fn a_child_gets_exactly_the_granted_provider_and_custom_secrets() {
    let env = env_of(&full_secrets(), &["anthropic", "custom:stripe_key"], &[]);
    assert_eq!(env.get("ANTHROPIC_API_KEY").map(String::as_str), Some("a"));
    assert_eq!(env.get("STRIPE_KEY").map(String::as_str), Some("sk_test"));
    assert!(!env.contains_key("OPENAI_API_KEY"));
    assert!(!env.contains_key("INTERCOM"));
    assert_eq!(
        env.get("SYSTEMPROMPT_CUSTOM_SECRETS").map(String::as_str),
        Some("STRIPE_KEY")
    );
}

#[test]
fn a_derived_grant_is_emitted_when_configured_and_skipped_when_not() {
    let env = env_of(&minimal_secrets(), &[], &["gemini"]);
    assert!(!env.contains_key("GEMINI_API_KEY"));
    let env = env_of(&full_secrets(), &[], &["gemini"]);
    assert_eq!(env.get("GEMINI_API_KEY").map(String::as_str), Some("g"));
}

#[test]
fn an_explicit_grant_that_is_not_configured_is_an_error() {
    let grants = ChildGrants {
        explicit: vec!["custom:absent".parse().expect("grant")],
        derived: Vec::new(),
    };
    assert_eq!(
        minimal_secrets().child_env(&grants),
        Err(ChildEnvError::NotConfigured {
            grant: "custom:absent".to_owned()
        })
    );
}
