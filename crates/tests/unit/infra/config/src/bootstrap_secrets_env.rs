use systemprompt_config::{ConfigError, ProfileBootstrap, SecretsBootstrap, SecretsBootstrapError};

use crate::fixture;

const ENV_DB_URL: &str = "postgresql://env:env@localhost:5432/env_secrets";

fn set_base_env() {
    fixture::set_env("OAUTH_AT_REST_PEPPER", fixture::PEPPER);
    fixture::set_env("DATABASE_URL", ENV_DB_URL);
    fixture::set_env("MANIFEST_SIGNING_SECRET_SEED", fixture::SEED);
}

#[test]
fn env_source_falls_back_to_environment_when_file_missing() {
    let fx = fixture::write_tree(fixture::ENV_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    set_base_env();
    fixture::set_env("DATABASE_WRITE_URL", "postgresql://w:w@localhost:5432/w");
    fixture::set_env("GEMINI_API_KEY", "gem-key");
    fixture::set_env("ANTHROPIC_API_KEY", "ant-key");

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.database_url, ENV_DB_URL);
    assert_eq!(
        secrets.database_write_url.as_deref(),
        Some("postgresql://w:w@localhost:5432/w")
    );
    assert_eq!(secrets.gemini.as_deref(), Some("gem-key"));
    assert_eq!(secrets.anthropic.as_deref(), Some("ant-key"));
    assert_eq!(
        secrets.manifest_signing_secret_seed.as_deref(),
        Some(fixture::SEED)
    );
}

#[test]
fn env_source_prefers_file_when_present() {
    let fx = fixture::write_tree(
        fixture::ENV_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    set_base_env();

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.database_url, fixture::DB_URL);
}

#[test]
fn env_source_missing_pepper_errors() {
    let fx = fixture::write_tree(fixture::ENV_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::remove_env("OAUTH_AT_REST_PEPPER");
    fixture::set_env("DATABASE_URL", ENV_DB_URL);

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::OauthAtRestPepperRequired)
    ));
}

#[test]
fn env_source_empty_database_url_errors() {
    let fx = fixture::write_tree(fixture::ENV_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env("OAUTH_AT_REST_PEPPER", fixture::PEPPER);
    fixture::set_env("MANIFEST_SIGNING_SECRET_SEED", fixture::SEED);
    fixture::set_env("DATABASE_URL", "");

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::DatabaseUrlRequired)
    ));
}

#[test]
fn fly_environment_loads_from_env_without_profile_secrets() {
    let fx = fixture::write_tree("", None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env("FLY_APP_NAME", "cov-fly-app");
    set_base_env();

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.database_url, ENV_DB_URL);
    assert_eq!(secrets.oauth_at_rest_pepper, fixture::PEPPER);
}

#[test]
fn fly_environment_without_seed_keeps_seed_absent() {
    let fx = fixture::write_tree("", None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env("FLY_APP_NAME", "cov-fly-app");
    fixture::set_env("OAUTH_AT_REST_PEPPER", fixture::PEPPER);
    fixture::set_env("DATABASE_URL", ENV_DB_URL);
    fixture::remove_env("MANIFEST_SIGNING_SECRET_SEED");

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.manifest_signing_secret_seed, None);
    assert!(matches!(
        SecretsBootstrap::manifest_signing_secret_seed().unwrap_err(),
        SecretsBootstrapError::ManifestSeedUnavailable
    ));
}

#[test]
fn fly_environment_with_env_source_and_no_pepper_errors() {
    let fx = fixture::write_tree(fixture::ENV_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env("FLY_APP_NAME", "cov-fly-app");
    fixture::remove_env("OAUTH_AT_REST_PEPPER");
    fixture::set_env("DATABASE_URL", ENV_DB_URL);

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::OauthAtRestPepperRequired)
    ));
}

#[test]
fn subprocess_mode_short_env_pepper_falls_back_to_file() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env("SYSTEMPROMPT_SUBPROCESS", "1");
    fixture::set_env("OAUTH_AT_REST_PEPPER", "short");

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.oauth_at_rest_pepper, fixture::PEPPER);
    assert_eq!(secrets.database_url, fixture::DB_URL);
}

#[test]
fn subprocess_mode_with_env_pepper_loads_env_secrets() {
    let fx = fixture::write_tree("", None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    fixture::set_env("SYSTEMPROMPT_SUBPROCESS", "1");
    set_base_env();
    fixture::set_env("OPENAI_API_KEY", "oai-key");
    fixture::set_env("GITHUB_TOKEN", "gh-token");
    fixture::set_env("EXTERNAL_DATABASE_URL", "postgresql://e:e@localhost:5432/e");
    fixture::set_env("INTERNAL_DATABASE_URL", "postgresql://i:i@localhost:5432/i");

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.openai.as_deref(), Some("oai-key"));
    assert_eq!(secrets.github.as_deref(), Some("gh-token"));
    assert_eq!(
        secrets.external_database_url.as_deref(),
        Some("postgresql://e:e@localhost:5432/e")
    );
    assert_eq!(
        secrets.internal_database_url.as_deref(),
        Some("postgresql://i:i@localhost:5432/i")
    );
}

#[test]
fn env_source_reads_moonshot_and_qwen_alias_keys() {
    let fx = fixture::write_tree(fixture::ENV_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    set_base_env();
    fixture::remove_env("MOONSHOT_API_KEY");
    fixture::set_env("KIMI_API_KEY", "kimi-key");
    fixture::remove_env("QWEN_API_KEY");
    fixture::set_env("DASHSCOPE_API_KEY", "dash-key");

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.moonshot.as_deref(), Some("kimi-key"));
    assert_eq!(secrets.qwen.as_deref(), Some("dash-key"));
}

#[test]
fn env_source_collects_custom_secrets_from_listed_keys() {
    let fx = fixture::write_tree(fixture::ENV_SECRETS, None);
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
    set_base_env();
    fixture::set_env("SYSTEMPROMPT_CUSTOM_SECRETS", "COV_ONE, COV_TWO,COV_ABSENT");
    fixture::set_env("COV_ONE", "one-value");
    fixture::set_env("COV_TWO", "two-value");
    fixture::remove_env("COV_ABSENT");

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.custom.len(), 2);
    assert_eq!(
        secrets.custom.get("COV_ONE").map(String::as_str),
        Some("one-value")
    );
    assert_eq!(
        secrets.custom.get("COV_TWO").map(String::as_str),
        Some("two-value")
    );
}
