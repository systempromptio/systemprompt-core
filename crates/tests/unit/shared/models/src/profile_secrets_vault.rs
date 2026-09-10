use systemprompt_models::profile::{
    SecretsConfig, SecretsSource, SecretsValidationMode, VaultAuth, VaultKeyRef, VaultSecretsConfig,
};

use crate::profile_services_sources::{errors_of, local_profile};

fn vault_config() -> VaultSecretsConfig {
    VaultSecretsConfig {
        address: "https://vault.example.com".to_owned(),
        mount: "secret".to_owned(),
        path: "systemprompt/prod".to_owned(),
        namespace: None,
        auth: VaultAuth::Token {
            token_env: "VAULT_TOKEN".to_owned(),
            token_file: None,
        },
        keys: Default::default(),
        ca_cert_path: None,
        timeout_secs: 10,
        retries: 3,
    }
}

fn vault_secrets() -> SecretsConfig {
    SecretsConfig {
        source: SecretsSource::Vault,
        validation: SecretsValidationMode::Strict,
        secrets_path: None,
        vault: Some(vault_config()),
    }
}

#[test]
fn file_source_requires_a_secrets_path() {
    let cfg = SecretsConfig {
        source: SecretsSource::File,
        validation: SecretsValidationMode::Warn,
        secrets_path: None,
        vault: None,
    };
    let err = cfg.validate().expect_err("file source without a path");
    assert!(err.to_string().contains("requires secrets.secrets_path"));
    assert!(cfg.secrets_path().is_err());
}

#[test]
fn file_source_with_a_path_validates_and_exposes_it() {
    let cfg = SecretsConfig {
        source: SecretsSource::File,
        validation: SecretsValidationMode::Warn,
        secrets_path: Some("./secrets.json".to_owned()),
        vault: None,
    };
    cfg.validate().expect("valid");
    assert_eq!(cfg.secrets_path().expect("path"), "./secrets.json");
}

#[test]
fn an_empty_secrets_path_is_treated_as_absent() {
    let cfg = SecretsConfig {
        source: SecretsSource::File,
        validation: SecretsValidationMode::Warn,
        secrets_path: Some(String::new()),
        vault: None,
    };
    assert!(cfg.validate().is_err());
    assert!(cfg.secrets_path().is_err());
}

#[test]
fn env_source_needs_no_secrets_path() {
    let cfg = SecretsConfig {
        source: SecretsSource::Env,
        validation: SecretsValidationMode::Strict,
        secrets_path: None,
        vault: None,
    };
    cfg.validate().expect("valid");
    assert!(cfg.secrets_path().is_err());
}

#[test]
fn vault_source_requires_a_vault_block() {
    let cfg = SecretsConfig {
        source: SecretsSource::Vault,
        validation: SecretsValidationMode::Strict,
        secrets_path: None,
        vault: None,
    };
    let err = cfg.validate().expect_err("vault without a block");
    assert!(err.to_string().contains("requires a secrets.vault block"));
}

#[test]
fn a_vault_block_on_a_file_source_is_rejected() {
    let cfg = SecretsConfig {
        source: SecretsSource::File,
        validation: SecretsValidationMode::Warn,
        secrets_path: Some("./secrets.json".to_owned()),
        vault: Some(vault_config()),
    };
    let err = cfg.validate().expect_err("vault block with file source");
    assert!(
        err.to_string()
            .contains("only valid with secrets.source 'vault'")
    );
}

#[test]
fn vault_source_with_a_block_validates() {
    vault_secrets().validate().expect("valid");
}

#[test]
fn yaml_defaults_fill_mount_timeout_and_retries() {
    let yaml = r#"
source: vault
vault:
  address: https://vault.example.com
  path: systemprompt/prod
  auth:
    method: token
"#;
    let cfg: SecretsConfig = serde_yaml::from_str(yaml).expect("parse");
    let vault = cfg.vault.as_ref().expect("vault block");
    assert_eq!(vault.mount, "secret");
    assert_eq!(vault.timeout_secs, 10);
    assert_eq!(vault.retries, 3);
    let VaultAuth::Token { token_env, .. } = &vault.auth else {
        panic!("expected token auth");
    };
    assert_eq!(token_env, "VAULT_TOKEN");
}

#[test]
fn approle_and_kubernetes_auth_round_trip_with_defaults() {
    let approle: VaultAuth = serde_yaml::from_str("method: approle\n").expect("parse approle");
    let VaultAuth::AppRole {
        role_id_env,
        secret_id_env,
        mount,
    } = &approle
    else {
        panic!("expected approle");
    };
    assert_eq!(role_id_env, "VAULT_ROLE_ID");
    assert_eq!(secret_id_env, "VAULT_SECRET_ID");
    assert_eq!(mount, "approle");
    assert_eq!(approle.method_name(), "approle");

    let k8s: VaultAuth =
        serde_yaml::from_str("method: kubernetes\nrole: systemprompt\n").expect("parse k8s");
    let VaultAuth::Kubernetes {
        role,
        jwt_path,
        mount,
    } = &k8s
    else {
        panic!("expected kubernetes");
    };
    assert_eq!(role, "systemprompt");
    assert_eq!(
        jwt_path,
        "/var/run/secrets/kubernetes.io/serviceaccount/token"
    );
    assert_eq!(mount, "kubernetes");
}

#[test]
fn kubernetes_auth_without_a_role_fails_to_parse() {
    assert!(serde_yaml::from_str::<VaultAuth>("method: kubernetes\n").is_err());
}

#[test]
fn an_unknown_auth_method_is_rejected() {
    assert!(serde_yaml::from_str::<VaultAuth>("method: aws\n").is_err());
}

#[test]
fn skip_verify_is_a_parse_error_not_a_downgrade() {
    let yaml = r#"
address: https://vault.example.com
path: systemprompt/prod
skip_verify: true
auth:
  method: token
"#;
    let err = serde_yaml::from_str::<VaultSecretsConfig>(yaml).expect_err("skip_verify rejected");
    assert!(err.to_string().contains("skip_verify"));
}

#[test]
fn key_overrides_round_trip() {
    let yaml = r#"
address: https://vault.example.com
path: systemprompt/prod
auth:
  method: token
keys:
  signing_key_pem:
    path: shared/identity
    field: signing_key_pem
"#;
    let cfg: VaultSecretsConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(
        cfg.keys.get("signing_key_pem"),
        Some(&VaultKeyRef {
            path: "shared/identity".to_owned(),
            field: "signing_key_pem".to_owned(),
        })
    );
}

#[test]
fn a_plain_http_vault_address_is_rejected() {
    let mut profile = local_profile();
    let mut secrets = vault_secrets();
    secrets.vault.as_mut().expect("vault").address = "http://vault.internal".to_owned();
    profile.secrets = Some(secrets);
    assert!(errors_of(&profile).contains("secrets.vault.address is not reachable"));
}

#[test]
fn a_timeout_outside_the_supported_range_is_rejected() {
    let mut profile = local_profile();
    let mut secrets = vault_secrets();
    secrets.vault.as_mut().expect("vault").timeout_secs = 0;
    profile.secrets = Some(secrets);
    assert!(errors_of(&profile).contains("timeout_secs must be between 1 and 120"));

    let mut profile = local_profile();
    let mut secrets = vault_secrets();
    secrets.vault.as_mut().expect("vault").timeout_secs = 121;
    profile.secrets = Some(secrets);
    assert!(errors_of(&profile).contains("timeout_secs must be between 1 and 120"));
}

#[test]
fn too_many_retries_are_rejected() {
    let mut profile = local_profile();
    let mut secrets = vault_secrets();
    secrets.vault.as_mut().expect("vault").retries = 11;
    profile.secrets = Some(secrets);
    assert!(errors_of(&profile).contains("retries must be at most 10"));
}

#[test]
fn an_empty_vault_path_is_rejected() {
    let mut profile = local_profile();
    let mut secrets = vault_secrets();
    secrets.vault.as_mut().expect("vault").path = String::new();
    profile.secrets = Some(secrets);
    assert!(errors_of(&profile).contains("secrets.vault.path is required"));
}

#[test]
fn an_empty_key_override_is_rejected() {
    let mut profile = local_profile();
    let mut secrets = vault_secrets();
    secrets.vault.as_mut().expect("vault").keys.insert(
        "github".to_owned(),
        VaultKeyRef {
            path: String::new(),
            field: "token".to_owned(),
        },
    );
    profile.secrets = Some(secrets);
    assert!(errors_of(&profile).contains("secrets.vault.keys.github"));
}

#[test]
fn a_well_formed_vault_profile_validates() {
    let mut profile = local_profile();
    profile.secrets = Some(vault_secrets());
    assert_eq!(errors_of(&profile), "");
}

#[test]
fn unknown_secrets_keys_are_rejected() {
    let yaml = "source: file\nsecrets_path: ./secrets.json\nrotate: true\n";
    assert!(serde_yaml::from_str::<SecretsConfig>(yaml).is_err());
}
