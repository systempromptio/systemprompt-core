//! A Vault-backed profile changes what a deploy is allowed to push.
//!
//! The instance fetches its own secrets at boot, so the deploy pushes the
//! bootstrap credentials and nothing else — never `secrets.json`, never the
//! JWT signing key.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use systemprompt_cli::cloud::deploy::pipeline::{
    DeploySecretsSource, bootstrap_env_names, collect_bootstrap_env,
};
use systemprompt_models::profile::{
    SecretsConfig, SecretsSource, SecretsValidationMode, VaultAuth, VaultSecretsConfig,
};

fn vault(auth: VaultAuth, address: &str, namespace: Option<&str>) -> VaultSecretsConfig {
    VaultSecretsConfig {
        address: address.to_owned(),
        mount: "secret".to_owned(),
        path: "systemprompt/prod".to_owned(),
        namespace: namespace.map(ToOwned::to_owned),
        auth,
        keys: BTreeMap::new(),
        ca_cert_path: None,
        timeout_secs: 10,
        retries: 3,
    }
}

fn config(source: SecretsSource, vault: Option<VaultSecretsConfig>) -> SecretsConfig {
    SecretsConfig {
        source,
        validation: SecretsValidationMode::Strict,
        secrets_path: Some("secrets.json".to_owned()),
        vault,
    }
}

#[test]
fn a_file_profile_still_pushes_the_secrets_file() {
    let path = PathBuf::from("/profiles/prod/secrets.json");
    let resolved =
        DeploySecretsSource::from_profile(Some(&config(SecretsSource::File, None)), path.clone());
    assert_eq!(resolved, DeploySecretsSource::EnvFromFile { path });
}

#[test]
fn a_profile_without_a_secrets_block_still_pushes_the_secrets_file() {
    let path = PathBuf::from("/profiles/prod/secrets.json");
    let resolved = DeploySecretsSource::from_profile(None, path.clone());
    assert_eq!(resolved, DeploySecretsSource::EnvFromFile { path });
}

#[test]
fn token_auth_pushes_only_the_token_variable() {
    let auth = VaultAuth::Token {
        token_env: "VAULT_TOKEN".to_owned(),
        token_file: None,
    };
    assert_eq!(
        bootstrap_env_names(&vault(auth, "https://vault.example.test", None)),
        vec!["VAULT_TOKEN".to_owned()]
    );
}

#[test]
fn a_token_file_needs_no_pushed_variable() {
    let auth = VaultAuth::Token {
        token_env: "VAULT_TOKEN".to_owned(),
        token_file: Some("/run/vault/token".to_owned()),
    };
    assert!(
        bootstrap_env_names(&vault(auth, "https://vault.example.test", None)).is_empty(),
        "a token read from a file must not be pushed as a secret"
    );
}

#[test]
fn approle_auth_pushes_both_identity_variables() {
    let auth = VaultAuth::AppRole {
        role_id_env: "VAULT_ROLE_ID".to_owned(),
        secret_id_env: "VAULT_SECRET_ID".to_owned(),
        mount: "approle".to_owned(),
    };
    assert_eq!(
        bootstrap_env_names(&vault(auth, "https://vault.example.test", None)),
        vec!["VAULT_ROLE_ID".to_owned(), "VAULT_SECRET_ID".to_owned()]
    );
}

#[test]
fn kubernetes_auth_pushes_nothing() {
    let auth = VaultAuth::Kubernetes {
        role: "systemprompt".to_owned(),
        jwt_path: "/var/run/secrets/token".to_owned(),
        mount: "kubernetes".to_owned(),
    };
    assert!(
        bootstrap_env_names(&vault(auth, "https://vault.example.test", None)).is_empty(),
        "the pod's service-account token is not a deploy secret"
    );
}

#[test]
fn a_placeholder_address_pushes_the_variable_it_names() {
    let auth = VaultAuth::Kubernetes {
        role: "systemprompt".to_owned(),
        jwt_path: "/var/run/secrets/token".to_owned(),
        mount: "kubernetes".to_owned(),
    };
    let names = bootstrap_env_names(&vault(auth, "${VAULT_ADDR}", Some("${VAULT_NAMESPACE}")));
    assert_eq!(
        names,
        vec!["VAULT_ADDR".to_owned(), "VAULT_NAMESPACE".to_owned()]
    );
}

#[test]
fn a_literal_address_pushes_no_address_variable() {
    let auth = VaultAuth::Token {
        token_env: "VAULT_TOKEN".to_owned(),
        token_file: None,
    };
    let names = bootstrap_env_names(&vault(auth, "https://vault.example.test", Some("team-uk")));
    assert_eq!(names, vec!["VAULT_TOKEN".to_owned()]);
}

#[test]
fn a_vault_profile_resolves_to_the_vault_variant() {
    let auth = VaultAuth::Token {
        token_env: "VAULT_TOKEN".to_owned(),
        token_file: None,
    };
    let resolved = DeploySecretsSource::from_profile(
        Some(&config(
            SecretsSource::Vault,
            Some(vault(auth, "https://vault.example.test", None)),
        )),
        PathBuf::from("/profiles/prod/secrets.json"),
    );
    assert_eq!(
        resolved,
        DeploySecretsSource::Vault {
            bootstrap_env: vec!["VAULT_TOKEN".to_owned()]
        }
    );
}

#[test]
fn bootstrap_collection_pushes_exactly_the_named_variables() {
    let env: HashMap<&str, &str> = HashMap::from([
        ("VAULT_TOKEN", "hvs.example"),
        ("ANTHROPIC_API_KEY", "must-not-be-pushed"),
    ]);
    let collected = collect_bootstrap_env(&["VAULT_TOKEN".to_owned()], |name| {
        env.get(name).map(|v| (*v).to_owned())
    })
    .expect("the token is set");

    assert_eq!(collected.len(), 1);
    assert_eq!(
        collected.get("VAULT_TOKEN").map(String::as_str),
        Some("hvs.example")
    );
    assert!(!collected.contains_key("ANTHROPIC_API_KEY"));
    assert!(!collected.contains_key("SIGNING_KEY_PEM"));
}

#[test]
fn a_missing_bootstrap_variable_names_itself_in_the_error() {
    let err = collect_bootstrap_env(
        &["VAULT_ROLE_ID".to_owned(), "VAULT_SECRET_ID".to_owned()],
        |name| (name == "VAULT_ROLE_ID").then(|| "role".to_owned()),
    )
    .expect_err("a missing bootstrap variable must fail the deploy");
    let message = err.to_string();
    assert!(message.contains("VAULT_SECRET_ID"), "{message}");
    assert!(!message.contains("VAULT_ROLE_ID"), "{message}");
}

#[test]
fn a_blank_bootstrap_variable_counts_as_missing() {
    let err = collect_bootstrap_env(&["VAULT_TOKEN".to_owned()], |_name| Some("   ".to_owned()))
        .expect_err("a blank value must fail the deploy");
    assert!(err.to_string().contains("VAULT_TOKEN"), "{err}");
}
