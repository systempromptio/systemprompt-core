use systemprompt_config::{ResolvedSource, SecretsBootstrapError, resolve_source};
use systemprompt_models::profile::{
    SecretsConfig, SecretsSource, SecretsValidationMode, VaultAuth, VaultSecretsConfig,
};

fn vault_block() -> VaultSecretsConfig {
    VaultSecretsConfig {
        address: "https://vault.example.com".to_owned(),
        mount: "secret".to_owned(),
        path: "systemprompt/prod".to_owned(),
        namespace: None,
        auth: VaultAuth::Token {
            token_env: "VAULT_TOKEN".to_owned(),
            token_file: None,
        },
        keys: std::collections::BTreeMap::new(),
        ca_cert_path: None,
        timeout_secs: 10,
        retries: 3,
    }
}

fn config(source: SecretsSource, path: Option<&str>, vault: bool) -> SecretsConfig {
    SecretsConfig {
        source,
        validation: SecretsValidationMode::Strict,
        secrets_path: path.map(str::to_owned),
        vault: vault.then(vault_block),
    }
}

#[test]
fn subprocess_with_pepper_wins_over_every_profile_source() {
    for source in [
        SecretsSource::File,
        SecretsSource::Env,
        SecretsSource::Vault,
    ] {
        let cfg = config(source, Some("secrets.json"), true);
        let resolved = resolve_source(Some(&cfg), true, false, true).unwrap();
        assert_eq!(resolved, ResolvedSource::SubprocessEnv);
    }
}

#[test]
fn subprocess_without_pepper_falls_through_to_the_profile() {
    let cfg = config(SecretsSource::File, Some("secrets.json"), false);
    let resolved = resolve_source(Some(&cfg), true, false, false).unwrap();
    assert_eq!(resolved, ResolvedSource::File("secrets.json"));
}

#[test]
fn vault_wins_on_a_deployment_host_even_with_a_pepper_present() {
    let cfg = config(SecretsSource::Vault, None, true);
    let resolved = resolve_source(Some(&cfg), false, true, true).unwrap();
    assert!(matches!(resolved, ResolvedSource::Vault(_)));
}

#[test]
fn vault_source_without_a_vault_block_is_an_error() {
    let cfg = config(SecretsSource::Vault, None, false);
    let err = resolve_source(Some(&cfg), false, false, false).unwrap_err();
    assert!(matches!(err, SecretsBootstrapError::VaultBlockMissing));
}

#[test]
fn deployment_host_uses_env_for_env_source_with_or_without_a_pepper() {
    let cfg = config(SecretsSource::Env, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), false, true, true).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
    assert_eq!(
        resolve_source(Some(&cfg), false, true, false).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
}

#[test]
fn deployment_host_with_a_pepper_overrides_a_file_source() {
    let cfg = config(SecretsSource::File, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), false, true, true).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
}

#[test]
fn deployment_host_without_a_pepper_still_reads_a_file_source() {
    let cfg = config(SecretsSource::File, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), false, true, false).unwrap(),
        ResolvedSource::File("secrets.json")
    );
}

#[test]
fn local_env_source_prefers_the_file_then_the_environment() {
    let cfg = config(SecretsSource::Env, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), false, false, false).unwrap(),
        ResolvedSource::LocalEnvWithFileFallback("secrets.json")
    );
}

#[test]
fn a_missing_secrets_path_is_reported_rather_than_substituted() {
    let cfg = config(SecretsSource::File, None, false);
    let err = resolve_source(Some(&cfg), false, false, false).unwrap_err();
    assert!(matches!(
        err,
        SecretsBootstrapError::SecretsConfigInvalid { .. }
    ));
}

#[test]
fn no_secrets_section_errors_unless_an_env_boot_can_serve_it() {
    let err = resolve_source(None, false, false, true).unwrap_err();
    assert!(matches!(err, SecretsBootstrapError::NoSecretsConfigured));

    assert_eq!(
        resolve_source(None, false, true, true).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
    assert_eq!(
        resolve_source(None, true, false, true).unwrap(),
        ResolvedSource::SubprocessEnv
    );
}
