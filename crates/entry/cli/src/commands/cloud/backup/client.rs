//! HTTP client for the tenant deployment's `/api/v1/sync/files` download
//! endpoints.
//!
//! Those routes require a `Service`-type JWT, so the operator's `api_token`
//! is exchanged via the RFC 8693 `token-exchange` grant against the
//! deployment's `/api/v1/core/oauth/token` endpoint. The grant requires a
//! registered client; `sp_web` is the seeded public client
//! (`token_endpoint_auth_method = none`), the same convention the API uses
//! for its own session flows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_SYNC_DEPLOY_TIMEOUT};

const TOKEN_EXCHANGE_GRANT: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const SUBJECT_TOKEN_TYPE_JWT: &str = "urn:ietf:params:oauth:token-type:jwt";
const PUBLIC_CLIENT_ID: &str = "sp_web";

#[derive(Debug, Deserialize)]
pub(super) struct BackupFileEntry {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct BackupManifest {
    pub files: Vec<BackupFileEntry>,
}

#[derive(Debug)]
pub(super) struct BackupClient {
    client: reqwest::Client,
    origin: String,
    bearer: String,
}

impl BackupClient {
    pub(super) async fn connect(hostname: &str, operator_token: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_SYNC_DEPLOY_TIMEOUT)
            .build()?;
        let origin = format!("https://{hostname}");
        let bearer = exchange_subject_token(&client, &origin, operator_token).await?;
        Ok(Self {
            client,
            origin,
            bearer,
        })
    }

    pub(super) async fn fetch_manifest(&self) -> Result<BackupManifest> {
        let url = format!("{}/api/v1/sync/files/manifest", self.origin);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.bearer)
            .send()
            .await
            .with_context(|| format!("GET {url} failed"))?;
        let status = response.status();
        if !status.is_success() {
            bail!("GET {url} returned {status}: {}", response.text().await?);
        }
        Ok(response.json().await?)
    }

    pub(super) async fn download_bundle(&self) -> Result<Vec<u8>> {
        let url = format!("{}/api/v1/sync/files", self.origin);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.bearer)
            .send()
            .await
            .with_context(|| format!("GET {url} failed"))?;
        let status = response.status();
        if !status.is_success() {
            bail!("GET {url} returned {status}: {}", response.text().await?);
        }
        Ok(response.bytes().await?.to_vec())
    }
}

async fn exchange_subject_token(
    client: &reqwest::Client,
    origin: &str,
    operator_token: &str,
) -> Result<String> {
    #[derive(Deserialize)]
    struct TokenExchangeResponse {
        access_token: String,
    }

    let url = format!("{origin}/api/v1/core/oauth/token");
    let response = client
        .post(&url)
        .form(&[
            ("grant_type", TOKEN_EXCHANGE_GRANT),
            ("subject_token", operator_token),
            ("subject_token_type", SUBJECT_TOKEN_TYPE_JWT),
            ("client_id", PUBLIC_CLIENT_ID),
        ])
        .send()
        .await
        .with_context(|| format!("POST {url} failed"))?;

    let status = response.status();
    if !status.is_success() {
        bail!(
            "token exchange at {url} returned {status}: {}",
            response.text().await?
        );
    }
    let parsed: TokenExchangeResponse = response.json().await?;
    Ok(parsed.access_token)
}
