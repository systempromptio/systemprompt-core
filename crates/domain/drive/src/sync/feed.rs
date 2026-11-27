//! The Drive sync feed under `/v1/bridge/drive/`: manifest, change feed, blobs,
//! commits, trash and moves.
//!
//! The feed takes its own credential: the device token the sign-in returned
//! beside the session token is traded for a short sync token, and every other
//! feed route accepts only that. A blob is handed back only after its digest
//! matched the address it was fetched by. A `404` from the manifest, the change
//! feed or the trade means the instance runs without a document store. A commit
//! or a trash names the revision it was based on in `If-Match`; a head that
//! moved on answers `409` with the head, decoded as a typed outcome.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use bytes::Bytes;
use chrono::Utc;
use reqwest::Method;
use serde::Deserialize;
use sp_client::{ClientError, HttpClient, TransportFailure};
use sp_protocol::Jwt;
use sp_protocol::ids::{DocumentId, DocumentRevisionId};
use sp_protocol::wire::drive::{
    CommitMeta, CommitOutcome, FeedChanges, FeedManifest, MoveRequest, SyncTokenAnswer,
    TrashOutcome, TrashRequest,
};
use sp_protocol::wire::paths;

use super::error::SyncError;
use crate::error::{decode_failure, plain_key, retry_after};
use crate::hash::sha256_hex;
use crate::http::{
    Answer, Call, DEFAULT_TIMEOUT, FormPart, Payload, SMALL_BODY, exchange, refusal, route,
};

const MANIFEST: &str = "GET /v1/bridge/drive/manifest";
const CHANGES: &str = "GET /v1/bridge/drive/changes";
const BLOB: &str = "GET /v1/bridge/drive/blob/{sha256}";
const COMMIT: &str = "POST /v1/bridge/drive/commit";
const TRASH: &str = "POST /v1/bridge/drive/trash";
const MOVE: &str = "POST /v1/bridge/drive/move";
pub(crate) const TOKEN: &str = "POST /v1/bridge/drive/token";

const CURSOR_RESET: &str = "cursor_reset";
const FEED_BODY: usize = 32 * 1024 * 1024;
const BLOB_BODY: usize = 512 * 1024 * 1024;
const BLOB_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub(crate) struct FeedClient {
    base: String,
    http: HttpClient,
}

impl FeedClient {
    pub(crate) const fn new(base: String, http: HttpClient) -> Self {
        Self { base, http }
    }

    pub(crate) async fn sync_token(&self, device: &Jwt) -> Result<SyncTokenAnswer, SyncError> {
        let call = Call::new(
            Method::POST,
            route(&self.base, paths::DRIVE_TOKEN, &[])?,
            TOKEN,
        );
        let answer = self.send(&call, device).await?;
        if answer.status == 404 {
            return Err(SyncError::Disabled { endpoint: TOKEN });
        }
        decode(TOKEN, &accept(TOKEN, answer)?)
    }

    pub(crate) async fn manifest(&self, token: &Jwt) -> Result<FeedManifest, SyncError> {
        let call = Call::new(
            Method::GET,
            route(&self.base, paths::DRIVE_MANIFEST, &[])?,
            MANIFEST,
        )
        .limits(DEFAULT_TIMEOUT, FEED_BODY);
        let answer = self.send(&call, token).await?;
        if answer.status == 404 {
            return Err(SyncError::Disabled { endpoint: MANIFEST });
        }
        decode(MANIFEST, &accept(MANIFEST, answer)?)
    }

    pub(crate) async fn changes(&self, token: &Jwt, since: &str) -> Result<FeedChanges, SyncError> {
        let mut url = route(&self.base, paths::DRIVE_CHANGES, &[])?;
        url.query_pairs_mut().append_pair("since", since);
        let call = Call::new(Method::GET, url, CHANGES).limits(DEFAULT_TIMEOUT, FEED_BODY);
        let answer = self.send(&call, token).await?;
        if answer.status == 404 {
            return Err(SyncError::Disabled { endpoint: CHANGES });
        }
        decode(CHANGES, &accept(CHANGES, answer)?)
    }

    pub(crate) async fn blob(&self, token: &Jwt, sha256: &str) -> Result<Vec<u8>, SyncError> {
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ClientError::InvalidInput {
                what: "a content address is not a SHA-256",
            }
            .into());
        }
        let call = Call::new(
            Method::GET,
            route(&self.base, paths::DRIVE_BLOB, &[sha256])?,
            BLOB,
        )
        .accept("application/octet-stream")
        .limits(BLOB_TIMEOUT, BLOB_BODY);
        let answer = accept(BLOB, self.send(&call, token).await?)?;
        let actual = sha256_hex(&answer.body);
        if !actual.eq_ignore_ascii_case(sha256) {
            return Err(SyncError::HashMismatch {
                expected: sha256.to_owned(),
                actual,
            });
        }
        Ok(answer.body)
    }

    pub(crate) async fn commit(
        &self,
        token: &Jwt,
        meta: &CommitMeta,
        blob: Bytes,
    ) -> Result<CommitOutcome, SyncError> {
        let meta_json = serde_json::to_string(meta).map_err(SyncError::Encode)?;
        let parts = vec![
            FormPart::Text {
                name: "meta",
                value: meta_json,
                mime: Some("application/json"),
            },
            FormPart::File {
                name: "blob",
                bytes: blob,
                file_name: meta.path.clone(),
                mime: "application/octet-stream".to_owned(),
            },
        ];
        let mut call = Call::new(
            Method::POST,
            route(&self.base, paths::DRIVE_COMMIT, &[])?,
            COMMIT,
        )
        .payload(Payload::Multipart(parts))
        .limits(BLOB_TIMEOUT, SMALL_BODY);
        if let Some(base) = &meta.base_revision {
            call = call.header("if-match", format!("\"{base}\""));
        }
        let answer = self.send(&call, token).await?;
        if answer.status == 409 {
            return decode(COMMIT, &answer);
        }
        decode(COMMIT, &accept(COMMIT, answer)?)
    }

    pub(crate) async fn trash(
        &self,
        token: &Jwt,
        document_id: &DocumentId,
        base_revision: &DocumentRevisionId,
    ) -> Result<TrashOutcome, SyncError> {
        let call = Call::new(
            Method::POST,
            route(&self.base, paths::DRIVE_TRASH, &[])?,
            TRASH,
        )
        .json(&TrashRequest { document_id })?
        .header("if-match", format!("\"{base_revision}\""));
        let answer = self.send(&call, token).await?;
        if answer.status == 409 {
            return decode(TRASH, &answer);
        }
        decode(TRASH, &accept(TRASH, answer)?)
    }

    pub(crate) async fn move_document(
        &self,
        token: &Jwt,
        document_id: &DocumentId,
        path: &str,
    ) -> Result<(), SyncError> {
        let url = route(&self.base, paths::DRIVE_MOVE, &[])?;
        let call = Call::new(Method::POST, url, MOVE).json(&MoveRequest { document_id, path })?;
        accept(MOVE, self.send(&call, token).await?).map(|_| ())
    }

    async fn send(&self, call: &Call, bearer: &Jwt) -> Result<Answer, SyncError> {
        let answer = exchange(self.http.reqwest(), call, bearer.expose()).await?;
        if answer.status == 401 {
            return Err(SyncError::Client(refusal(call.endpoint, &answer.body)));
        }
        Ok(answer)
    }
}

impl SyncError {
    pub(crate) fn is_cursor_reset(&self) -> bool {
        matches!(self, Self::Rejected { status: 410, error_key: Some(key), .. } if key == CURSOR_RESET)
    }
}

#[derive(Deserialize)]
struct RefusalBody {
    #[serde(default)]
    error_key: Option<String>,
}

fn accept(endpoint: &'static str, answer: Answer) -> Result<Answer, SyncError> {
    match answer.status {
        200..=299 => Ok(answer),
        401 => Err(SyncError::Client(refusal(endpoint, &answer.body))),
        429 => Err(SyncError::Client(ClientError::RateLimited {
            endpoint,
            retry_after: retry_after(&answer.headers, Utc::now()),
        })),
        status @ 500.. => Err(SyncError::Client(ClientError::Unreachable {
            endpoint,
            cause: TransportFailure::Status { status },
        })),
        status => {
            let error_key = serde_json::from_slice::<RefusalBody>(&answer.body)
                .ok()
                .and_then(|body| plain_key(body.error_key.as_deref()));
            tracing::debug!(
                endpoint,
                status,
                error_key = error_key.as_deref(),
                "the Drive feed refused a request"
            );
            Err(SyncError::Rejected {
                endpoint,
                status,
                error_key,
            })
        },
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    endpoint: &'static str,
    answer: &Answer,
) -> Result<T, SyncError> {
    serde_json::from_slice(&answer.body)
        .map_err(|error| SyncError::Client(decode_failure(endpoint, answer.status, &error)))
}
