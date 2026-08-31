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

#[test]
fn parse_treats_blank_provider_keys_as_absent() {
    let json = format!(
        r#"{{
            "oauth_at_rest_pepper": "{}",
            "database_url": "postgres://primary",
            "gemini": "g",
            "anthropic": "",
            "openai": "   ",
            "github": null
        }}"#,
        "p".repeat(32)
    );
    let secrets = Secrets::parse(&json).unwrap();
    assert_eq!(secrets.gemini.as_deref(), Some("g"));
    assert_eq!(secrets.anthropic, None);
    assert_eq!(secrets.openai, None);
    assert_eq!(secrets.github, None);
    assert!(secrets.has_ai_provider());
}

#[test]
fn none_if_blank_filters_empty_and_whitespace() {
    use systemprompt_models::none_if_blank;
    assert_eq!(none_if_blank(None), None);
    assert_eq!(none_if_blank(Some(String::new())), None);
    assert_eq!(none_if_blank(Some("  ".to_owned())), None);
    assert_eq!(
        none_if_blank(Some("key".to_owned())).as_deref(),
        Some("key")
    );
}

// Why: `effective_database_url` decides which database the process connects
// to. Picking the wrong one does not fail loudly — it connects successfully to
// somewhere the caller did not intend, which is worse than not connecting.
#[test]
fn external_db_access_selects_the_external_url_when_one_is_configured() {
    let secrets = full_secrets();

    assert_eq!(
        secrets.effective_database_url(true),
        "postgres://external",
        "with external access the external URL is the one that reaches the database"
    );
    assert_eq!(
        secrets.effective_database_url(false),
        "postgres://primary",
        "without external access the primary URL is used"
    );
}

// Why: the flag alone must not select an absent URL. Falling through to the
// primary is what lets a profile carry the flag without an external URL yet.
#[test]
fn external_db_access_falls_back_to_the_primary_when_no_external_url_exists() {
    let secrets = minimal_secrets();

    assert_eq!(
        secrets.effective_database_url(true),
        "postgres://primary",
        "an absent external URL must fall back rather than yield an empty target"
    );
}

// Why: the pepper protects at-rest hashes. A short one is guessable, and this
// is the only place that refuses it — the generator produces 64 characters,
// but a hand-edited secrets file can carry anything.
#[test]
fn a_pepper_below_the_minimum_length_is_refused() {
    let mut secrets = minimal_secrets();
    secrets.oauth_at_rest_pepper = "p".repeat(31);

    let err = secrets
        .validate()
        .expect_err("a 31-character pepper is below the enforced minimum");

    assert!(
        format!("{err}").contains("oauth_at_rest_pepper"),
        "the refusal should name the field: {err}"
    );
}

#[test]
fn a_pepper_at_exactly_the_minimum_length_is_accepted() {
    let mut secrets = minimal_secrets();
    secrets.oauth_at_rest_pepper = "p".repeat(32);

    secrets
        .validate()
        .expect("32 characters is the minimum, not one above it");
}

// Why: `get` is the lookup every consumer goes through, and each secret is
// reachable by two spellings. A missing alias reads as an unset secret, so the
// caller proceeds without it rather than failing.
#[test]
fn each_secret_is_reachable_by_both_its_snake_case_and_env_var_name() {
    let secrets = full_secrets();

    for (snake, env) in [
        ("oauth_at_rest_pepper", "OAUTH_AT_REST_PEPPER"),
        ("database_url", "DATABASE_URL"),
        ("external_database_url", "EXTERNAL_DATABASE_URL"),
        ("gemini", "GEMINI_API_KEY"),
        ("anthropic", "ANTHROPIC_API_KEY"),
        ("openai", "OPENAI_API_KEY"),
        ("github", "GITHUB_TOKEN"),
    ] {
        assert_eq!(
            secrets.get(snake),
            secrets.get(env),
            "{snake} and {env} must name the same secret"
        );
        assert!(secrets.get(snake).is_some(), "{snake} should be set");
    }
}

// Why: moonshot and qwen each answer to a vendor alias as well. An operator
// naming the vendor gets the same secret as one naming the provider.
#[test]
fn vendor_aliases_resolve_to_the_same_secret() {
    let secrets = full_secrets();

    assert_eq!(secrets.get("moonshot"), secrets.get("kimi"));
    assert_eq!(secrets.get("moonshot"), secrets.get("KIMI_API_KEY"));
    assert_eq!(secrets.get("qwen"), secrets.get("dashscope"));
    assert_eq!(secrets.get("qwen"), secrets.get("DASHSCOPE_API_KEY"));
}

// Why: custom secrets are looked up case-insensitively in one direction only —
// the stored casing is tried first, then the opposite. Without it an operator
// who wrote `STRIPE_KEY` in the file and asked for `stripe_key` gets nothing.
#[test]
fn a_custom_secret_is_found_whichever_case_it_was_stored_in() {
    let secrets = full_secrets();

    assert_eq!(
        secrets.get("stripe_key").map(String::as_str),
        Some("sk_test"),
        "a key stored uppercase must be reachable in lower case"
    );
    assert_eq!(
        secrets.get("INTERCOM").map(String::as_str),
        Some("ic_value"),
        "a key stored lowercase must be reachable in upper case"
    );
}

#[test]
fn an_unknown_secret_is_absent_rather_than_empty() {
    assert!(minimal_secrets().get("no_such_secret").is_none());
}

// Why: `has_ai_provider` gates whether AI features are offered at all. Reading
// true with nothing configured means every request fails at the provider call
// instead of the feature being withheld.
#[test]
fn has_ai_provider_is_false_only_when_every_provider_is_absent() {
    assert!(!minimal_secrets().has_ai_provider());
    assert!(full_secrets().has_ai_provider());

    for set_one in [
        |s: &mut Secrets| s.gemini = Some("k".to_owned()),
        |s: &mut Secrets| s.anthropic = Some("k".to_owned()),
        |s: &mut Secrets| s.openai = Some("k".to_owned()),
        |s: &mut Secrets| s.moonshot = Some("k".to_owned()),
        |s: &mut Secrets| s.qwen = Some("k".to_owned()),
    ] {
        let mut secrets = minimal_secrets();
        set_one(&mut secrets);
        assert!(
            secrets.has_ai_provider(),
            "one configured provider is enough"
        );
    }
}
