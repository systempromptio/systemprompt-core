//! Vault preflight for a profile whose secrets come from KV v2.
//!
//! The deploy pushes only bootstrap credentials for such a profile, so a
//! misconfigured mount or an unreadable document is invisible until the
//! container fails to boot. These two checks move that failure to the
//! operator's terminal, before an image is built.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_config::{SecretsProvider, VaultKvProvider};
use systemprompt_models::profile::VaultSecretsConfig;

use super::CheckResult;

pub fn check_vault_address(vault: &VaultSecretsConfig) -> CheckResult {
    let Ok(url) = url::Url::parse(&vault.address) else {
        return CheckResult::fail(
            "vault-address",
            format!("secrets.vault.address is not a URL: {}", vault.address),
        );
    };
    if url.scheme() != "https" && !is_loopback_host(url.host_str()) {
        return CheckResult::fail(
            "vault-address",
            format!(
                "secrets.vault.address must be https outside loopback, got {}",
                vault.address
            ),
        );
    }
    if url.host_str().is_none() {
        return CheckResult::fail(
            "vault-address",
            format!("secrets.vault.address has no host: {}", vault.address),
        );
    }
    CheckResult::pass("vault-address", vault.address.clone())
}

pub async fn check_vault_document(
    vault: &VaultSecretsConfig,
) -> (CheckResult, HashMap<String, String>) {
    let provider = match VaultKvProvider::from_config(vault, |name| std::env::var(name).ok()) {
        Ok(provider) => provider,
        Err(err) => {
            return (
                CheckResult::fail("vault-document", err.to_string()),
                HashMap::new(),
            );
        },
    };

    let document = match provider.fetch().await {
        Ok(document) => document,
        Err(err) => {
            return (
                CheckResult::fail("vault-document", err.to_string()),
                HashMap::new(),
            );
        },
    };

    let names = document.key_names();
    let detail = format!(
        "document {}/{} readable via {}, {} keys",
        vault.mount,
        vault.path,
        vault.auth.method_name(),
        names.len()
    );

    match document.into_secrets() {
        Ok(secrets) => {
            let values = names
                .iter()
                .filter_map(|name| secrets.get(name).map(|v| (name.clone(), v.clone())))
                .collect();
            (CheckResult::pass("vault-document", detail), values)
        },
        Err(err) => {
            let placeholders = names
                .into_iter()
                .map(|name| (name, String::new()))
                .collect();
            (
                CheckResult::fail(
                    "vault-document",
                    format!("{detail}, but the document does not parse: {err}"),
                ),
                placeholders,
            )
        },
    }
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost" | "127.0.0.1" | "::1"))
}
