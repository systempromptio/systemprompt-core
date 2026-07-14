use base64::Engine;
use systemprompt_config::{
    ConfigError, MANIFEST_SIGNING_SEED_BYTES, ProfileBootstrap, SecretsBootstrap,
    SecretsBootstrapError, decode_seed,
};

use crate::fixture;

fn init_profile(fx: &fixture::Fixture) {
    ProfileBootstrap::init_from_path(&fx.profile_path).unwrap();
}

#[test]
fn init_loads_secrets_from_file_with_seed() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    init_profile(&fx);

    let secrets = SecretsBootstrap::init().unwrap();

    assert_eq!(secrets.oauth_at_rest_pepper, fixture::PEPPER);
    assert_eq!(secrets.database_url, fixture::DB_URL);
    assert_eq!(SecretsBootstrap::database_url().unwrap(), fixture::DB_URL);
    assert_eq!(SecretsBootstrap::database_write_url().unwrap(), None);
    assert_eq!(
        SecretsBootstrap::oauth_at_rest_pepper().unwrap(),
        fixture::PEPPER
    );
    assert_eq!(
        SecretsBootstrap::manifest_signing_secret_seed().unwrap(),
        [0u8; MANIFEST_SIGNING_SEED_BYTES]
    );
    assert!(SecretsBootstrap::is_initialized());
}

#[test]
fn init_errors_before_profile_bootstrap() {
    let err = SecretsBootstrap::init().unwrap_err();
    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::ProfileNotInitialized)
    ));
}

#[test]
fn init_errors_when_profile_has_no_secrets_section() {
    let fx = fixture::write_tree("", None);
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::NoSecretsConfigured)
    ));
}

#[test]
fn init_errors_when_secrets_file_missing_strict() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, None);
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    match err {
        ConfigError::Secrets(SecretsBootstrapError::FileNotFound { path }) => {
            assert_eq!(path, fx.secrets_path.display().to_string());
        },
        other => panic!("expected FileNotFound, got: {other:?}"),
    }
}

#[test]
fn init_errors_when_secrets_file_missing_warn_mode() {
    let section = "secrets:\n  secrets_path: secrets.json\n  source: file\n  validation: warn\n";
    let fx = fixture::write_tree(section, None);
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::FileNotFound { .. })
    ));
}

#[test]
fn init_errors_when_secrets_file_missing_skip_mode() {
    let section = "secrets:\n  secrets_path: secrets.json\n  source: file\n  validation: skip\n";
    let fx = fixture::write_tree(section, None);
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::FileNotFound { .. })
    ));
}

#[test]
fn init_errors_on_invalid_secrets_json() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some("not json"));
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    match err {
        ConfigError::Secrets(SecretsBootstrapError::InvalidSecretsFile { message }) => {
            assert!(
                message.contains("Failed to parse secrets JSON"),
                "{message}"
            );
        },
        other => panic!("expected InvalidSecretsFile, got: {other:?}"),
    }
}

#[test]
fn init_errors_on_short_pepper() {
    let body = format!(
        "{{\"oauth_at_rest_pepper\": \"short\", \"database_url\": \"{}\"}}",
        fixture::DB_URL
    );
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&body));
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    match err {
        ConfigError::Secrets(SecretsBootstrapError::InvalidSecretsFile { message }) => {
            assert!(message.contains("oauth_at_rest_pepper"), "{message}");
        },
        other => panic!("expected InvalidSecretsFile, got: {other:?}"),
    }
}

#[test]
fn init_errors_on_invalid_manifest_seed() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some("not-base64!!!"))),
    );
    init_profile(&fx);

    let err = SecretsBootstrap::init().unwrap_err();

    match err {
        ConfigError::Secrets(SecretsBootstrapError::ManifestSeedInvalid { message }) => {
            assert!(message.contains("base64 decode failed"), "{message}");
        },
        other => panic!("expected ManifestSeedInvalid, got: {other:?}"),
    }
}

#[test]
fn init_generates_and_persists_missing_seed() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&fixture::secrets_json(None)));
    init_profile(&fx);

    let secrets = SecretsBootstrap::init().unwrap();

    let encoded = secrets.manifest_signing_secret_seed.as_deref().unwrap();
    let in_memory = decode_seed(encoded).unwrap();

    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fx.secrets_path).unwrap()).unwrap();
    let persisted = decode_seed(on_disk["manifest_signing_secret_seed"].as_str().unwrap()).unwrap();
    assert_eq!(persisted, in_memory);
    assert_eq!(
        on_disk["oauth_at_rest_pepper"].as_str(),
        Some(fixture::PEPPER)
    );
    assert_eq!(
        SecretsBootstrap::manifest_signing_secret_seed().unwrap(),
        in_memory
    );
}

#[cfg(unix)]
#[test]
fn init_uses_ephemeral_seed_when_profile_dir_read_only() {
    use std::os::unix::fs::PermissionsExt;

    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&fixture::secrets_json(None)));
    init_profile(&fx);
    let dir = fx.profile_path.parent().unwrap();
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();

    let secrets = SecretsBootstrap::init().unwrap();

    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let encoded = secrets.manifest_signing_secret_seed.as_deref().unwrap();
    decode_seed(encoded).unwrap();
    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fx.secrets_path).unwrap()).unwrap();
    assert!(
        on_disk.get("manifest_signing_secret_seed").is_none(),
        "read-only dir must not be written"
    );
}

#[test]
fn init_errors_in_subprocess_mode_without_seed() {
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&fixture::secrets_json(None)));
    init_profile(&fx);
    fixture::set_env("SYSTEMPROMPT_SUBPROCESS", "1");
    fixture::remove_env("OAUTH_AT_REST_PEPPER");

    let err = SecretsBootstrap::init().unwrap_err();

    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::SubprocessSeedMissing)
    ));
}

#[test]
fn accessors_error_before_init() {
    assert!(matches!(
        SecretsBootstrap::get().unwrap_err(),
        SecretsBootstrapError::NotInitialized
    ));
    assert!(matches!(
        SecretsBootstrap::database_url().unwrap_err(),
        SecretsBootstrapError::NotInitialized
    ));
    assert!(!SecretsBootstrap::is_initialized());
}

#[test]
fn try_init_is_idempotent() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    init_profile(&fx);

    let first = SecretsBootstrap::try_init().unwrap();
    let second = SecretsBootstrap::try_init().unwrap();
    assert!(std::ptr::eq(first, second));

    let err = SecretsBootstrap::init().unwrap_err();
    assert!(matches!(
        err,
        ConfigError::Secrets(SecretsBootstrapError::AlreadyInitialized)
    ));
    assert!(std::ptr::eq(SecretsBootstrap::require().unwrap(), first));
}

#[test]
fn signing_key_pem_round_trips_and_rejects_bad_encodings() {
    let pem = "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----\n";
    let encoded = base64::engine::general_purpose::STANDARD.encode(pem);
    let body = format!(
        "{{\"oauth_at_rest_pepper\": \"{}\", \"database_url\": \"{}\", \
         \"manifest_signing_secret_seed\": \"{}\", \"signing_key_pem\": \"{encoded}\"}}",
        fixture::PEPPER,
        fixture::DB_URL,
        fixture::SEED
    );
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&body));
    init_profile(&fx);
    SecretsBootstrap::init().unwrap();

    assert_eq!(
        SecretsBootstrap::signing_key_pem().unwrap().as_deref(),
        Some(pem)
    );
}

#[test]
fn signing_key_pem_invalid_base64_errors() {
    let body = format!(
        "{{\"oauth_at_rest_pepper\": \"{}\", \"database_url\": \"{}\", \
         \"manifest_signing_secret_seed\": \"{}\", \"signing_key_pem\": \"%%%not-b64\"}}",
        fixture::PEPPER,
        fixture::DB_URL,
        fixture::SEED
    );
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&body));
    init_profile(&fx);
    SecretsBootstrap::init().unwrap();

    let err = SecretsBootstrap::signing_key_pem().unwrap_err();
    assert!(matches!(
        err,
        SecretsBootstrapError::SigningKeyPemInvalid { .. }
    ));
}

#[test]
fn signing_key_pem_invalid_utf8_errors() {
    let encoded = base64::engine::general_purpose::STANDARD.encode([0xff, 0xfe, 0x00, 0x9f]);
    let body = format!(
        "{{\"oauth_at_rest_pepper\": \"{}\", \"database_url\": \"{}\", \
         \"manifest_signing_secret_seed\": \"{}\", \"signing_key_pem\": \"{encoded}\"}}",
        fixture::PEPPER,
        fixture::DB_URL,
        fixture::SEED
    );
    let fx = fixture::write_tree(fixture::FILE_SECRETS, Some(&body));
    init_profile(&fx);
    SecretsBootstrap::init().unwrap();

    let err = SecretsBootstrap::signing_key_pem().unwrap_err();
    match err {
        SecretsBootstrapError::SigningKeyPemInvalid { message } => {
            assert!(message.contains("utf-8"), "{message}");
        },
        other => panic!("expected SigningKeyPemInvalid, got: {other:?}"),
    }
}

#[test]
fn signing_key_pem_absent_returns_none() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    init_profile(&fx);
    SecretsBootstrap::init().unwrap();

    assert_eq!(SecretsBootstrap::signing_key_pem().unwrap(), None);
}

#[test]
fn rotate_manifest_signing_seed_persists_new_seed() {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&fixture::secrets_json(Some(fixture::SEED))),
    );
    init_profile(&fx);
    SecretsBootstrap::init().unwrap();

    let rotated = SecretsBootstrap::rotate_manifest_signing_seed().unwrap();

    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fx.secrets_path).unwrap()).unwrap();
    let persisted = decode_seed(on_disk["manifest_signing_secret_seed"].as_str().unwrap()).unwrap();
    assert_eq!(persisted, rotated);
    assert_ne!(persisted, [0u8; MANIFEST_SIGNING_SEED_BYTES]);
    assert_eq!(
        on_disk["oauth_at_rest_pepper"].as_str(),
        Some(fixture::PEPPER)
    );
}
