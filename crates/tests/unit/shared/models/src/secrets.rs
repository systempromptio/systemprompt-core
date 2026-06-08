use std::collections::HashMap;
use systemprompt_models::secrets::Secrets;

fn full_secrets() -> Secrets {
    let mut custom = HashMap::new();
    custom.insert("STRIPE_KEY".to_owned(), "sk_test".to_owned());
    custom.insert("intercom".to_owned(), "ic_value".to_owned());
    Secrets {
        oauth_at_rest_pepper: "p".repeat(32),
        manifest_signing_secret_seed: Some("seed".to_owned()),
        signing_key_pem: None,
        database_url: "postgres://primary".to_owned(),
        database_write_url: Some("postgres://write".to_owned()),
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
        database_url: "postgres://primary".to_owned(),
        database_write_url: None,
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

#[test]
fn to_subprocess_env_includes_required_fields() {
    let env: HashMap<String, String> = minimal_secrets().to_subprocess_env().into_iter().collect();
    assert!(env.contains_key("OAUTH_AT_REST_PEPPER"));
    assert!(env.contains_key("DATABASE_URL"));
}

#[test]
fn to_subprocess_env_omits_absent_optionals() {
    let env: HashMap<String, String> = minimal_secrets().to_subprocess_env().into_iter().collect();
    for key in [
        "MANIFEST_SIGNING_SECRET_SEED",
        "DATABASE_WRITE_URL",
        "EXTERNAL_DATABASE_URL",
        "INTERNAL_DATABASE_URL",
        "GEMINI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "MOONSHOT_API_KEY",
        "QWEN_API_KEY",
        "SYSTEMPROMPT_CUSTOM_SECRETS",
    ] {
        assert!(!env.contains_key(key), "expected {key} absent");
    }
}

#[test]
fn to_subprocess_env_emits_all_optionals_when_present() {
    let env: HashMap<String, String> = full_secrets().to_subprocess_env().into_iter().collect();
    assert_eq!(env.get("MANIFEST_SIGNING_SECRET_SEED").unwrap(), "seed");
    assert_eq!(env.get("DATABASE_WRITE_URL").unwrap(), "postgres://write");
    assert_eq!(
        env.get("EXTERNAL_DATABASE_URL").unwrap(),
        "postgres://external"
    );
    assert_eq!(
        env.get("INTERNAL_DATABASE_URL").unwrap(),
        "postgres://internal"
    );
    assert_eq!(env.get("GEMINI_API_KEY").unwrap(), "g");
    assert_eq!(env.get("ANTHROPIC_API_KEY").unwrap(), "a");
    assert_eq!(env.get("OPENAI_API_KEY").unwrap(), "o");
    assert_eq!(env.get("GITHUB_TOKEN").unwrap(), "gh");
    assert_eq!(env.get("MOONSHOT_API_KEY").unwrap(), "m");
    assert_eq!(env.get("QWEN_API_KEY").unwrap(), "q");
}

#[test]
fn to_subprocess_env_emits_custom_index_and_upper_case_keys() {
    let env: HashMap<String, String> = full_secrets().to_subprocess_env().into_iter().collect();
    let index = env.get("SYSTEMPROMPT_CUSTOM_SECRETS").unwrap();
    let names: std::collections::HashSet<&str> = index.split(',').collect();
    assert!(names.contains("STRIPE_KEY"));
    assert!(names.contains("INTERCOM"));
    assert_eq!(env.get("STRIPE_KEY").unwrap(), "sk_test");
    assert_eq!(env.get("INTERCOM").unwrap(), "ic_value");
    assert_eq!(env.get("intercom").unwrap(), "ic_value");
}
