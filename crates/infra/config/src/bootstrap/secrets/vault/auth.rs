//! Vault login methods producing a short-lived client token.
//!
//! `lease_duration` is carried on [`VaultSession`] and deliberately unused in
//! phase 1, where rotation is a process restart. It is the anchor for the
//! phase-2 refresh loop, so it is parsed and kept rather than discarded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_models::profile::VaultAuth;
use zeroize::Zeroizing;

use super::EnvLookup;
use super::client::VaultHttp;
use super::error::{VaultError, truncate_detail};

#[derive(Debug)]
pub(super) struct VaultSession {
    pub(super) token: Zeroizing<String>,
    pub(super) lease_duration: u64,
}

#[derive(Deserialize)]
struct LoginResponse {
    auth: LoginAuth,
}

#[derive(Deserialize)]
struct LoginAuth {
    client_token: String,

    #[serde(default)]
    lease_duration: u64,
}

pub(super) async fn login(
    http: &VaultHttp,
    auth: &VaultAuth,
    lookup_env: &EnvLookup,
) -> Result<VaultSession, VaultError> {
    match auth {
        VaultAuth::Token {
            token_env,
            token_file,
        } => static_token(token_env, token_file.as_deref(), lookup_env),
        VaultAuth::AppRole {
            role_id_env,
            secret_id_env,
            mount,
        } => {
            let role_id = require_env(role_id_env, lookup_env)?;
            let secret_id = require_env(secret_id_env, lookup_env)?;
            let body =
                serde_json::json!({ "role_id": role_id.as_str(), "secret_id": secret_id.as_str() });
            post_login(http, mount, "approle", &body).await
        },
        VaultAuth::Kubernetes {
            role,
            jwt_path,
            mount,
        } => {
            let jwt =
                std::fs::read_to_string(jwt_path).map_err(|e| VaultError::CredentialFile {
                    path: jwt_path.clone(),
                    message: e.to_string(),
                })?;
            let body = serde_json::json!({ "role": role, "jwt": jwt.trim() });
            post_login(http, mount, "kubernetes", &body).await
        },
    }
}

fn static_token(
    token_env: &str,
    token_file: Option<&str>,
    lookup_env: &EnvLookup,
) -> Result<VaultSession, VaultError> {
    if let Some(token) = lookup_env(token_env).filter(|v| !v.trim().is_empty()) {
        return Ok(VaultSession {
            token: Zeroizing::new(token.trim().to_owned()),
            lease_duration: 0,
        });
    }

    let path = token_file.ok_or_else(|| VaultError::MissingCredential {
        name: token_env.to_owned(),
    })?;
    let raw = std::fs::read_to_string(path).map_err(|e| VaultError::CredentialFile {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    let token = raw.trim();
    if token.is_empty() {
        return Err(VaultError::CredentialFile {
            path: path.to_owned(),
            message: "file is empty".to_owned(),
        });
    }
    Ok(VaultSession {
        token: Zeroizing::new(token.to_owned()),
        lease_duration: 0,
    })
}

fn require_env(name: &str, lookup_env: &EnvLookup) -> Result<Zeroizing<String>, VaultError> {
    lookup_env(name)
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .map(Zeroizing::new)
        .ok_or_else(|| VaultError::MissingCredential {
            name: name.to_owned(),
        })
}

async fn post_login(
    http: &VaultHttp,
    mount: &str,
    method: &'static str,
    body: &serde_json::Value,
) -> Result<VaultSession, VaultError> {
    let path = format!("/v1/auth/{mount}/login");
    let response = http
        .send_with_retry(|| {
            http.request(reqwest::Method::POST, &path)
                .map(|b| b.json(body))
        })
        .await?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(VaultError::Auth {
            method,
            status: status.as_u16(),
            detail: truncate_detail(&vault_errors(&text)),
        });
    }

    let parsed: LoginResponse = serde_json::from_str(&text).map_err(|e| VaultError::Malformed {
        message: format!("{method} login response: {e}"),
    })?;

    Ok(VaultSession {
        token: Zeroizing::new(parsed.auth.client_token),
        lease_duration: parsed.auth.lease_duration,
    })
}

pub(super) fn vault_errors(body: &str) -> String {
    #[derive(Deserialize)]
    struct Errors {
        #[serde(default)]
        errors: Vec<String>,
    }

    serde_json::from_str::<Errors>(body)
        .map(|e| e.errors.join("; "))
        .unwrap_or_default()
}
