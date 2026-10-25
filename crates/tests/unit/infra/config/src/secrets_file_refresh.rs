//! `secrets.refresh_secs` for a file source, the boot-required key list shared
//! with `secret check`, and the replica configuration fingerprint.

use std::time::Duration;

use systemprompt_config::{
    BOOT_REQUIRED_SECRET_KEYS, ConfigFingerprint, LoadedProfile, SIGNING_KEY_PEM, SecretsService,
    refresh_interval_for,
};
use systemprompt_manifest::profile::{SecretsConfig, SecretsSource, SecretsValidationMode};

use crate::fixture;

const FILE_REFRESH_SECRETS: &str =
    "secrets:\n  secrets_path: secrets.json\n  source: file\n  refresh_secs: 60\n";

fn file_config(refresh_secs: Option<u64>) -> SecretsConfig {
    SecretsConfig {
        source: SecretsSource::File,
        validation: SecretsValidationMode::Warn,
        secrets_path: Some("secrets.json".to_owned()),
        vault: None,
        external: None,
        refresh_secs,
    }
}

#[test]
fn a_file_source_with_refresh_secs_has_a_refresh_interval() {
    assert_eq!(
        refresh_interval_for(&file_config(Some(120))),
        Some(Duration::from_secs(120))
    );
    assert_eq!(refresh_interval_for(&file_config(None)), None);
}

#[test]
fn refresh_secs_below_the_floor_is_rejected() {
    let err = file_config(Some(59)).validate().expect_err("below 60 s");
    assert!(err.to_string().contains("secrets.refresh_secs"), "{err}");
    file_config(Some(60)).validate().expect("60 s is the floor");
}

#[test]
fn refresh_secs_is_refused_for_a_non_file_source() {
    let mut config = file_config(Some(60));
    config.source = SecretsSource::Env;
    let err = config.validate().expect_err("env source with refresh_secs");
    assert!(
        err.to_string()
            .contains("only valid with secrets.source 'file'"),
        "{err}"
    );
}

#[test]
fn the_boot_required_keys_include_the_encryption_master_key() {
    for key in [
        "database_url",
        "oauth_at_rest_pepper",
        "manifest_signing_secret_seed",
        "encryption_master_key",
    ] {
        assert!(BOOT_REQUIRED_SECRET_KEYS.contains(&key), "{key}");
    }
    assert!(!SecretsService::required_secret_keys(false).contains(&SIGNING_KEY_PEM));
    assert!(SecretsService::required_secret_keys(true).contains(&SIGNING_KEY_PEM));
}

#[tokio::test]
async fn a_rotated_secrets_file_is_picked_up_by_a_reload() {
    let body = |provider: &str| {
        let base = fixture::secrets_json(Some(fixture::SEED));
        base.replacen('{', &format!("{{\n  \"anthropic\": \"{provider}\","), 1)
    };
    let fx = fixture::write_tree(FILE_REFRESH_SECRETS, Some(&body("key-one")));
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let service = SecretsService::load(profile.clone())
        .await
        .expect("secrets");
    assert_eq!(service.refresh_interval(), Some(Duration::from_secs(60)));
    let before = service.generation();
    let fingerprint_before =
        ConfigFingerprint::current(profile.profile(), &service, None).expect("fingerprint");

    std::fs::write(&fx.secrets_path, body("key-two")).expect("rotate");
    let outcome = service.reload().await.expect("reload");

    assert!(outcome.swapped);
    assert_eq!(outcome.changed_keys, vec!["anthropic"]);
    assert_eq!(service.generation(), before + 1);
    let fingerprint_after =
        ConfigFingerprint::current(profile.profile(), &service, None).expect("fingerprint");
    assert_eq!(fingerprint_after.secrets_generation, before + 1);
    assert_eq!(
        fingerprint_after.profile_sha,
        fingerprint_before.profile_sha
    );
    assert_eq!(
        fingerprint_after.secret_keys_sha,
        fingerprint_before.secret_keys_sha
    );
}

#[tokio::test]
async fn the_fingerprint_names_keys_but_never_exposes_values() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let service = SecretsService::load(profile.clone())
        .await
        .expect("secrets");

    let fingerprint = ConfigFingerprint::current(profile.profile(), &service, Some("f".repeat(64)))
        .expect("fingerprint");
    let rendered = serde_json::to_string(&fingerprint).expect("serialises");
    for value in [
        fixture::PEPPER,
        fixture::DB_URL,
        fixture::MASTER_KEY,
        fixture::SEED,
    ] {
        assert!(
            !rendered.contains(value),
            "a secret value leaked: {rendered}"
        );
    }
    assert_eq!(fingerprint.profile_sha.len(), 16);
    assert_eq!(
        fingerprint.services_sha.as_deref(),
        Some("ffffffffffffffff")
    );
    assert!(
        service
            .key_names()
            .expect("names")
            .contains(&"encryption_master_key".to_owned())
    );
}

#[test]
fn different_secret_names_give_different_fingerprints() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    let loaded = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let profile = loaded.profile();
    let a =
        ConfigFingerprint::compute(profile, &["a".to_owned(), "b".to_owned()], 1, None).expect("a");
    let b =
        ConfigFingerprint::compute(profile, &["b".to_owned(), "a".to_owned()], 1, None).expect("b");
    let c = ConfigFingerprint::compute(profile, &["a".to_owned()], 1, None).expect("c");
    assert_eq!(a, b);
    assert_ne!(a.secret_keys_sha, c.secret_keys_sha);
    assert_eq!(a.profile_sha, c.profile_sha);
}
