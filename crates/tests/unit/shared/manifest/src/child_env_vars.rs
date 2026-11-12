use systemprompt_manifest::child_env::{EnvVarRefusal, refused_env_var};
use systemprompt_manifest::profile::SecretsConfig;
use systemprompt_manifest::secrets::Secrets;

fn source(yaml: &str) -> SecretsConfig {
    serde_yaml::from_str(yaml).expect("secrets config parses")
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

fn renamed_token() -> SecretsConfig {
    source(
        "source: vault\nvault:\n  address: https://vault.example.com\n  path: systemprompt\n  \
         auth:\n    method: token\n    token_env: OPS_BOOT_CREDENTIAL\n",
    )
}

#[test]
fn the_default_vault_token_is_refused_with_no_profile_in_hand() {
    let err = refused_env_var(None, &names(&["RUST_LOG", "VAULT_TOKEN"]))
        .expect_err("the default Vault token reads the whole secrets document");
    assert_eq!(
        err,
        EnvVarRefusal::CarriesSecret {
            name: "VAULT_TOKEN".to_owned()
        }
    );
}

#[test]
fn a_renamed_vault_token_env_is_refused() {
    let err = refused_env_var(
        Some(&renamed_token()),
        &names(&["RUST_LOG", "ops_boot_credential"]),
    )
    .expect_err("the renamed token env is the Vault credential");
    assert!(err.to_string().contains("ops_boot_credential"), "{err}");
    refused_env_var(Some(&renamed_token()), &names(&["RUST_LOG"]))
        .expect("a plain pass-through stays allowed");
}

#[test]
fn renamed_approle_credentials_are_refused() {
    let config = source(
        "source: vault\nvault:\n  address: https://vault.example.com\n  path: systemprompt\n  \
         auth:\n    method: approle\n    role_id_env: OPS_ROLE\n    secret_id_env: OPS_SECRET\n",
    );
    for name in ["OPS_ROLE", "OPS_SECRET"] {
        refused_env_var(Some(&config), &names(&[name])).expect_err(name);
    }
}

#[test]
fn an_external_source_refuses_every_pass_through() {
    let config = source("source: external\nexternal:\n  kind: example\n");
    let err = refused_env_var(Some(&config), &names(&["RUST_LOG"]))
        .expect_err("an external provider's env names cannot be enumerated");
    assert_eq!(
        err,
        EnvVarRefusal::ExternalSource {
            name: "RUST_LOG".to_owned()
        }
    );
    refused_env_var(Some(&config), &[]).expect("no pass-through, nothing to refuse");
}

#[test]
fn a_custom_secret_in_the_loaded_document_is_refused() {
    let secrets: Secrets = serde_json::from_value(serde_json::json!({
        "oauth_at_rest_pepper": "p".repeat(64),
        "database_url": "postgres://localhost/db",
        "PARTNER_WEBHOOK_SECRET": "s3cr3t",
    }))
    .expect("secrets document parses");
    secrets
        .refused_env_var(None, &names(&["partner_webhook_secret"]))
        .expect_err("a custom secret is granted under secrets:, never passed through");
    secrets
        .refused_env_var(None, &names(&["RUST_LOG"]))
        .expect("a plain pass-through stays allowed");
}
