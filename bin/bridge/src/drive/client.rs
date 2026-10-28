//! HTTP client for the bridge Drive routes under `/v1/bridge/drive/`.
//!
//! Each call takes the bearer it runs under, so the caller decides how a
//! token is minted and what a 401 does to it. Every route but
//! [`DriveClient::sync_token`] takes the Drive sync token that call trades
//! the bridge token for. A downloaded blob is returned
//! only after its digest matched the address it was requested by. A 404 from
//! the manifest or changes route means the server runs with the document
//! store disabled, and is reported as [`DriveError::ServerDisabled`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use reqwest::StatusCode;
use reqwest::multipart::{Form, Part};
use systemprompt_identifiers::{ContentHash, ValidatedUrl};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::{
    DriveChanges, DriveCommitMeta, DriveCommitResponse, DriveManifest, DriveProposals,
    DriveSyncToken, PresenceBeacon,
};

use super::error::{DriveError, DriveResult};
use super::hash;
use crate::ids::BearerToken;

#[derive(Debug, Clone)]
pub struct DriveClient {
    base_url: ValidatedUrl,
    http: reqwest::Client,
}

impl DriveClient {
    #[must_use]
    pub const fn new(base_url: ValidatedUrl, http: reqwest::Client) -> Self {
        Self { base_url, http }
    }

    #[must_use]
    pub const fn base_url(&self) -> &ValidatedUrl {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path)
    }

    pub async fn manifest(&self, bearer: &BearerToken) -> DriveResult<DriveManifest> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_MANIFEST;
        let request = self
            .http
            .get(self.url(endpoint))
            .bearer_auth(bearer.expose());
        decode(
            endpoint,
            accept(
                endpoint,
                store_enabled(endpoint, send(endpoint, request).await?)?,
            )
            .await?,
        )
        .await
    }

    pub async fn changes(&self, bearer: &BearerToken, since: &str) -> DriveResult<DriveChanges> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_CHANGES;
        let since: String = url::form_urlencoded::byte_serialize(since.as_bytes()).collect();
        let request = self
            .http
            .get(format!("{}?since={since}", self.url(endpoint)))
            .bearer_auth(bearer.expose());
        decode(
            endpoint,
            accept(
                endpoint,
                store_enabled(endpoint, send(endpoint, request).await?)?,
            )
            .await?,
        )
        .await
    }

    pub async fn download_blob(
        &self,
        bearer: &BearerToken,
        sha256: &ContentHash,
    ) -> DriveResult<Bytes> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_BLOB_ROUTE;
        let request = self
            .http
            .get(self.url(&ApiPaths::bridge_drive_blob(sha256)))
            .bearer_auth(bearer.expose());
        let response = accept(endpoint, send(endpoint, request).await?).await?;
        let bytes = response
            .bytes()
            .await
            .map_err(|source| DriveError::Network { endpoint, source })?;
        hash::verify(sha256, &bytes)?;
        Ok(bytes)
    }

    pub async fn commit(
        &self,
        bearer: &BearerToken,
        meta: &DriveCommitMeta,
        blob: Vec<u8>,
    ) -> DriveResult<DriveCommitResponse> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_COMMIT;
        let meta_json = serde_json::to_string(meta).map_err(DriveError::Encode)?;
        let meta_part = Part::text(meta_json)
            .mime_str("application/json")
            .map_err(|source| DriveError::Network { endpoint, source })?;
        let blob_part = Part::bytes(blob)
            .file_name(meta.path.clone())
            .mime_str("application/octet-stream")
            .map_err(|source| DriveError::Network { endpoint, source })?;
        let form = Form::new().part("meta", meta_part).part("blob", blob_part);
        let mut request = self
            .http
            .post(self.url(endpoint))
            .bearer_auth(bearer.expose())
            .multipart(form);
        if let Some(base) = &meta.base_revision {
            request = request.header(reqwest::header::IF_MATCH, format!("\"{}\"", base.as_str()));
        }
        let response = send(endpoint, request).await?;
        if response.status() == StatusCode::CONFLICT {
            return decode(endpoint, response).await;
        }
        decode(endpoint, accept(endpoint, response).await?).await
    }

    /// Trades the bridge's private device token for the Drive sync's own
    /// credential, the only token the other Drive routes accept.
    pub async fn sync_token(&self, device: &BearerToken) -> DriveResult<DriveSyncToken> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_TOKEN;
        let request = self
            .http
            .post(self.url(endpoint))
            .bearer_auth(device.expose());
        decode(
            endpoint,
            accept(
                endpoint,
                store_enabled(endpoint, send(endpoint, request).await?)?,
            )
            .await?,
        )
        .await
    }

    pub async fn proposals(&self, bearer: &BearerToken) -> DriveResult<DriveProposals> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_PROPOSALS;
        let request = self
            .http
            .get(self.url(endpoint))
            .bearer_auth(bearer.expose());
        decode(
            endpoint,
            accept(endpoint, send(endpoint, request).await?).await?,
        )
        .await
    }

    pub async fn presence(&self, bearer: &BearerToken, beacon: &PresenceBeacon) -> DriveResult<()> {
        let endpoint = ApiPaths::BRIDGE_DRIVE_PRESENCE;
        let request = self
            .http
            .put(self.url(endpoint))
            .bearer_auth(bearer.expose())
            .json(beacon);
        accept(endpoint, send(endpoint, request).await?).await?;
        Ok(())
    }
}

async fn send(
    endpoint: &'static str,
    request: reqwest::RequestBuilder,
) -> DriveResult<reqwest::Response> {
    request
        .send()
        .await
        .map_err(|source| DriveError::Network { endpoint, source })
}

fn store_enabled(
    endpoint: &'static str,
    response: reqwest::Response,
) -> DriveResult<reqwest::Response> {
    if response.status() == StatusCode::NOT_FOUND {
        return Err(DriveError::ServerDisabled { endpoint });
    }
    Ok(response)
}

async fn accept(
    endpoint: &'static str,
    response: reqwest::Response,
) -> DriveResult<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err(DriveError::Unauthorized { endpoint });
    }
    let (status, rejection) = crate::gateway::GatewayRejection::read(response).await;
    Err(DriveError::Rejected {
        endpoint,
        status,
        rejection,
    })
}

async fn decode<T: serde::de::DeserializeOwned>(
    endpoint: &'static str,
    response: reqwest::Response,
) -> DriveResult<T> {
    let bytes = response
        .bytes()
        .await
        .map_err(|source| DriveError::Network { endpoint, source })?;
    serde_json::from_slice(&bytes).map_err(|source| DriveError::Decode { endpoint, source })
}
