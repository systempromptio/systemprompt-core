//! KV v2 reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use zeroize::Zeroizing;

use super::client::VaultHttp;
use super::error::{VaultError, truncate_detail};

#[derive(Deserialize)]
struct KvReadResponse {
    data: KvReadData,
}

#[derive(Deserialize)]
struct KvReadData {
    data: serde_json::Map<String, serde_json::Value>,

    #[serde(default)]
    metadata: KvMetadata,
}

#[derive(Deserialize, Default)]
struct KvMetadata {
    #[serde(default)]
    version: u64,
}

pub(super) struct KvEntry {
    pub(super) fields: serde_json::Map<String, serde_json::Value>,
    pub(super) version: u64,
}

pub(super) async fn read_entry(
    http: &VaultHttp,
    token: &Zeroizing<String>,
    mount: &str,
    path: &str,
) -> Result<KvEntry, VaultError> {
    let request_path = format!(
        "/v1/{}/data/{}",
        mount.trim_matches('/'),
        path.trim_matches('/')
    );

    let response = http
        .send_with_retry(|| {
            http.request(Method::GET, &request_path)
                .map(|b| b.header("X-Vault-Token", token.as_str()))
        })
        .await?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let detail = truncate_detail(&super::auth::vault_errors(&text));

    match status {
        StatusCode::NOT_FOUND => {
            return Err(VaultError::NotFound {
                mount: mount.to_owned(),
                path: path.to_owned(),
            });
        },
        StatusCode::FORBIDDEN => {
            return Err(VaultError::Forbidden {
                mount: mount.to_owned(),
                path: path.to_owned(),
                detail,
            });
        },
        s if !s.is_success() => {
            return Err(VaultError::Http {
                status: s.as_u16(),
                mount: mount.to_owned(),
                path: path.to_owned(),
                detail,
            });
        },
        _ => {},
    }

    let parsed: KvReadResponse =
        serde_json::from_str(&text).map_err(|e| VaultError::Malformed {
            message: format!("{mount}/{path}: {e}"),
        })?;

    Ok(KvEntry {
        fields: parsed.data.data,
        version: parsed.data.metadata.version,
    })
}
