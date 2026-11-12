use systemprompt_config::{ResolvedSource, SecretsBootstrapError, resolve_source};
use systemprompt_manifest::profile::{
    ExternalSecretsConfig, SecretsConfig, SecretsSource, SecretsValidationMode, VaultAuth,
    VaultSecretsConfig,
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
        external: None,
        refresh_secs: None,
    }
}

#[test]
fn a_pepper_in_a_local_environment_never_overrides_the_profile_source() {
    let cfg = config(SecretsSource::File, Some("secrets.json"), false);
    let resolved = resolve_source(Some(&cfg), false, true).unwrap();
    assert_eq!(resolved, ResolvedSource::File("secrets.json"));
}

#[test]
fn vault_wins_on_a_deployment_host_even_with_a_pepper_present() {
    let cfg = config(SecretsSource::Vault, None, true);
    let resolved = resolve_source(Some(&cfg), true, true).unwrap();
    assert!(matches!(resolved, ResolvedSource::Vault(_)));
}

#[test]
fn vault_source_without_a_vault_block_is_an_error() {
    let cfg = config(SecretsSource::Vault, None, false);
    let err = resolve_source(Some(&cfg), false, false).unwrap_err();
    assert!(matches!(err, SecretsBootstrapError::VaultBlockMissing));
}

#[test]
fn deployment_host_uses_env_for_env_source_with_or_without_a_pepper() {
    let cfg = config(SecretsSource::Env, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), true, true).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
    assert_eq!(
        resolve_source(Some(&cfg), true, false).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
}

#[test]
fn an_explicit_file_source_reads_the_file_on_a_deployment_host_with_a_pepper() {
    let cfg = config(
        SecretsSource::File,
        Some("/etc/systemprompt/secrets.json"),
        false,
    );
    assert_eq!(
        resolve_source(Some(&cfg), true, true).unwrap(),
        ResolvedSource::File("/etc/systemprompt/secrets.json")
    );
}

#[test]
fn deployment_host_without_a_pepper_still_reads_a_file_source() {
    let cfg = config(SecretsSource::File, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), true, false).unwrap(),
        ResolvedSource::File("secrets.json")
    );
}

#[test]
fn local_env_source_prefers_the_file_then_the_environment() {
    let cfg = config(SecretsSource::Env, Some("secrets.json"), false);
    assert_eq!(
        resolve_source(Some(&cfg), false, false).unwrap(),
        ResolvedSource::LocalEnvWithFileFallback("secrets.json")
    );
}

#[test]
fn a_missing_secrets_path_is_reported_rather_than_substituted() {
    let cfg = config(SecretsSource::File, None, false);
    let err = resolve_source(Some(&cfg), false, false).unwrap_err();
    assert!(matches!(
        err,
        SecretsBootstrapError::SecretsConfigInvalid(_)
    ));
}

#[test]
fn no_secrets_section_errors_unless_an_env_boot_can_serve_it() {
    let err = resolve_source(None, false, true).unwrap_err();
    assert!(matches!(err, SecretsBootstrapError::NoSecretsConfigured));

    assert_eq!(
        resolve_source(None, true, true).unwrap(),
        ResolvedSource::DeploymentHostEnv
    );
}

fn external_config() -> SecretsConfig {
    let mut cfg = config(SecretsSource::External, None, false);
    cfg.external = Some(ExternalSecretsConfig {
        kind: "stub".to_owned(),
        refresh_secs: Some(300),
        config: serde_json::Map::new(),
    });
    cfg
}

#[test]
fn external_source_wins_on_deployment_hosts_too() {
    let cfg = external_config();
    for (is_deployment_host, has_pepper) in [(false, false), (true, false), (true, true)] {
        let resolved = resolve_source(Some(&cfg), is_deployment_host, has_pepper).unwrap();
        assert!(
            matches!(resolved, ResolvedSource::External(external) if external.kind == "stub"),
            "deployment_host={is_deployment_host} pepper={has_pepper}: {resolved:?}"
        );
    }
}

#[test]
fn external_source_without_its_block_fails_closed() {
    let cfg = config(SecretsSource::External, None, false);
    let err = resolve_source(Some(&cfg), true, true).unwrap_err();
    assert!(matches!(err, SecretsBootstrapError::ExternalBlockMissing));
}
