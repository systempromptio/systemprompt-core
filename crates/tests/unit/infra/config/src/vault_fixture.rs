use std::collections::BTreeMap;

use systemprompt_models::profile::{VaultAuth, VaultKeyRef, VaultSecretsConfig};

pub const PEPPER: &str = "vault_test_oauth_at_rest_pepper_value_32";
pub const SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
pub const DB_URL: &str = "postgresql://user:pass@localhost:5432/vault_fixture";

pub fn token_auth() -> VaultAuth {
    VaultAuth::Token {
        token_env: "VAULT_TOKEN".to_owned(),
        token_file: None,
    }
}

pub fn config(address: &str, auth: VaultAuth) -> VaultSecretsConfig {
    VaultSecretsConfig {
        address: address.to_owned(),
        mount: "secret".to_owned(),
        path: "systemprompt/prod".to_owned(),
        namespace: None,
        auth,
        keys: BTreeMap::new(),
        ca_cert_path: None,
        timeout_secs: 5,
        retries: 3,
    }
}

pub fn key_override(path: &str, field: &str) -> VaultKeyRef {
    VaultKeyRef {
        path: path.to_owned(),
        field: field.to_owned(),
    }
}

pub fn kv_body(data: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "data": { "data": data, "metadata": { "version": 4 } }
    })
}

pub fn secrets_document() -> serde_json::Value {
    serde_json::json!({
        "oauth_at_rest_pepper": PEPPER,
        "database_url": DB_URL,
        "manifest_signing_secret_seed": SEED,
        "gemini": serde_json::Value::Null,
    })
}

pub fn set_env(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value) };
}

pub fn remove_env(key: &str) {
    unsafe { std::env::remove_var(key) };
}
