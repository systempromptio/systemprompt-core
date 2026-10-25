use std::path::PathBuf;

use std::sync::Arc;
use systemprompt_config::{
    ConfigError, LoadedProfile, SecretsService, build_from_profile, validate_database_config,
};
use systemprompt_manifest::Config;

use crate::fixture;

struct Booted {
    profile: Arc<LoadedProfile>,
    secrets: Arc<SecretsService>,
}

impl Booted {
    fn build(&self) -> Result<Config, ConfigError> {
        build_from_profile(&self.profile, &self.secrets.get(), None)
    }
}

async fn boot(fx: &fixture::Fixture) -> Booted {
    let profile = LoadedProfile::from_path(&fx.profile_path).unwrap();
    let secrets = SecretsService::load(profile.clone()).await.unwrap();
    Booted { profile, secrets }
}

fn file_fixture() -> fixture::Fixture {
    fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    )
}

#[tokio::test]
async fn build_from_profile_resolves_paths_and_secrets() {
    let fx = file_fixture();
    let booted = boot(&fx).await;

    let config = booted.build().unwrap();

    let root = std::fs::canonicalize(fx.tmp.path()).unwrap();
    assert_eq!(
        config.system_path,
        root.join("system").display().to_string()
    );
    assert_eq!(config.database_url, fixture::DB_URL);
    assert_eq!(config.database_write_url, None);
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
    assert_eq!(config.system_admin_username, "testadmin");
    assert_eq!(
        config.signing_key_path,
        root.join("system").join("signing_key.pem")
    );
    assert_eq!(
        config.github_link,
        "https://github.com/systemprompt/systemprompt-os"
    );
}

#[tokio::test]
async fn build_from_profile_reports_missing_paths() {
    let fx = file_fixture();
    let booted = boot(&fx).await;
    std::fs::remove_dir_all(fx.tmp.path().join("bin")).unwrap();

    let err = booted.build().unwrap_err();

    match err {
        ConfigError::ProfilePathReport { message } => {
            assert!(message.contains("paths.bin"), "{message}");
            assert!(message.contains("Path does not exist"), "{message}");
        },
        other => panic!("expected ProfilePathReport, got: {other:?}"),
    }
}

#[tokio::test]
async fn build_from_profile_rejects_invalid_yaml_config_file() {
    let fx = file_fixture();
    let booted = boot(&fx).await;
    std::fs::write(
        fx.tmp.path().join("services/web/config.yaml"),
        ": not: [valid yaml",
    )
    .unwrap();

    let err = booted.build().unwrap_err();

    match err {
        ConfigError::InvalidProfileYaml { field, path, .. } => {
            assert_eq!(field, "web_config");
            assert_eq!(path, fx.tmp.path().join("services/web/config.yaml"));
        },
        other => panic!("expected InvalidProfileYaml, got: {other:?}"),
    }
}

#[tokio::test]
async fn build_from_profile_reports_unreadable_yaml_path() {
    let fx = file_fixture();
    let booted = boot(&fx).await;
    let metadata = fx.tmp.path().join("services/web/metadata.yaml");
    std::fs::remove_file(&metadata).unwrap();
    std::fs::create_dir(&metadata).unwrap();

    let err = booted.build().unwrap_err();

    match err {
        ConfigError::ReadProfilePath { field, path, .. } => {
            assert_eq!(field, "web_metadata");
            assert_eq!(path, PathBuf::from(&metadata));
        },
        other => panic!("expected ReadProfilePath, got: {other:?}"),
    }
}

#[tokio::test]
async fn system_admin_env_override_wins_over_profile() {
    let fx = file_fixture();
    let booted = boot(&fx).await;
    fixture::set_env("SYSTEMPROMPT_SYSTEM_ADMIN", "  env_admin  ");

    let config = booted.build().unwrap();

    assert_eq!(config.system_admin_username, "env_admin");
}

#[tokio::test]
async fn blank_system_admin_without_override_errors() {
    let fx = file_fixture();
    let yaml = std::fs::read_to_string(&fx.profile_path).unwrap();
    std::fs::write(
        &fx.profile_path,
        yaml.replace("username: testadmin", "username: ' '"),
    )
    .unwrap();
    let booted = boot(&fx).await;
    fixture::remove_env("SYSTEMPROMPT_SYSTEM_ADMIN");

    let err = booted.build().unwrap_err();

    assert!(matches!(err, ConfigError::MissingSystemAdmin));
}

#[tokio::test]
async fn absolute_signing_key_path_is_kept_verbatim() {
    let fx = file_fixture();
    let yaml = std::fs::read_to_string(&fx.profile_path).unwrap();
    std::fs::write(
        &fx.profile_path,
        yaml.replace(
            "signing_key_path: signing_key.pem",
            "signing_key_path: /etc/keys/sp.pem",
        ),
    )
    .unwrap();
    let booted = boot(&fx).await;

    let config = booted.build().unwrap();

    assert_eq!(config.signing_key_path, PathBuf::from("/etc/keys/sp.pem"));
}

#[tokio::test]
async fn validate_database_config_rejects_unsupported_type() {
    let fx = file_fixture();
    let booted = boot(&fx).await;
    let mut config = booted.build().unwrap();
    config.database_type = "mysql".to_owned();

    let err = validate_database_config(&config).unwrap_err();

    match err {
        ConfigError::UnsupportedDatabaseType { db_type } => assert_eq!(db_type, "mysql"),
        other => panic!("expected UnsupportedDatabaseType, got: {other:?}"),
    }
}

#[tokio::test]
async fn validate_database_config_rejects_invalid_urls() {
    let fx = file_fixture();
    let booted = boot(&fx).await;
    let good = booted.build().unwrap();

    let mut bad_read = good.clone();
    bad_read.database_url = "mysql://nope".to_owned();
    assert!(matches!(
        validate_database_config(&bad_read).unwrap_err(),
        ConfigError::InvalidDatabaseUrl { .. }
    ));

    let mut bad_write = good.clone();
    bad_write.database_write_url = Some("not-a-url".to_owned());
    assert!(matches!(
        validate_database_config(&bad_write).unwrap_err(),
        ConfigError::InvalidDatabaseUrl { .. }
    ));

    let mut postgresql_type = good;
    postgresql_type.database_type = "PostgreSQL".to_owned();
    validate_database_config(&postgresql_type).unwrap();
}

#[tokio::test]
async fn blank_instance_id_falls_back_to_generated_default() {
    let fx = file_fixture();
    let yaml = std::fs::read_to_string(&fx.profile_path).unwrap();
    std::fs::write(
        &fx.profile_path,
        yaml.replace("instance_id: null", "instance_id: '  '"),
    )
    .unwrap();
    let booted = boot(&fx).await;

    let config = booted.build().unwrap();

    assert!(!config.instance_id.as_str().trim().is_empty());
}
