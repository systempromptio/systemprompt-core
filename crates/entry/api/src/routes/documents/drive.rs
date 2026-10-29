//! The bridge Drive feed under `/v1/bridge/drive/`: manifest, change feed,
//! blob download, commit, presence and open proposals.
//!
//! Every route but `token` takes only the Drive sync token (audience
//! `bridge.drive_sync`, see `drive_token`) and answers 401 for anything else,
//! a plain bridge token included; `token` trades the bridge token for it. Blobs
//! are resolved through the document service, so a hash opens content only for
//! a caller who can view a document holding it. A commit is
//! `multipart/form-data` with a `meta` part ([`DriveCommitMeta`]) and a `blob`
//! part; an `If-Match` header names the base revision and must agree with
//! `meta.base_revision` when both are sent. A stale base answers 409 with the
//! head revision and its content hash.
//!
//! The Drive sync is a bearer client, so every route runs as an agent named
//! and attended from the bridge token's signed claims, on the `drive_sync`
//! channel, under the document's agent cap; a whole-file commit is a
//! `replace_content` write the agent access policy must allow, and may allow
//! for this channel alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::agents::act_as;
use systemprompt_files::documents::service::parse_drive_cursor;
use systemprompt_identifiers::{ContentHash, TraceId};
use systemprompt_models::ApiPaths;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::documents::{
    DriveChanges, DriveCommitMeta, DriveCommitResponse, DriveManifest, DriveProposal,
    DriveProposals, PresenceBeacon,
};

use super::drive_token::device_user;
use super::error::DocumentsResult;
use super::headers::{base_revision, etag, none_match, range};
use crate::services::middleware::JwtContextExtractor;

const DEFAULT_CHANGES: i64 = 500;

/// Router-scoped state of the bridge Drive routes: the document service,
/// the credential decoder and the authority that mints sync tokens.
#[derive(Clone)]
pub struct DriveState {
    service: Arc<DocumentService>,
    jwt: Arc<JwtContextExtractor>,
    tokens: Arc<dyn TokenAuthority>,
}

impl std::fmt::Debug for DriveState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DriveState").finish_non_exhaustive()
    }
}

impl DriveState {
    #[must_use]
    pub fn new(
        service: Arc<DocumentService>,
        jwt: Arc<JwtContextExtractor>,
        tokens: Arc<dyn TokenAuthority>,
    ) -> Self {
        Self {
            service,
            jwt,
            tokens,
        }
    }

    pub(super) fn jwt(&self) -> &JwtContextExtractor {
        &self.jwt
    }

    pub(super) fn tokens(&self) -> &dyn TokenAuthority {
        self.tokens.as_ref()
    }
}

pub fn router(state: DriveState) -> Router {
    let limit = super::body_limit(&state.service);
    Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_MANIFEST),
            get(manifest),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_CHANGES),
            get(changes),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_BLOB_ROUTE),
            get(blob),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_COMMIT),
            post(commit),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_PRESENCE),
            put(presence),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_TOKEN),
            post(super::drive_token::issue),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DRIVE_PROPOSALS),
            get(proposals),
        )
        .layer(DefaultBodyLimit::max(limit))
        .with_state(state)
}


async fn manifest(
    State(state): State<DriveState>,
    headers: HeaderMap,
) -> DocumentsResult<Json<DriveManifest>> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    Ok(Json(
        act_as(actor, state.service.drive_manifest(&who, &trace)).await?,
    ))
}

#[derive(Debug, Deserialize)]
struct ChangesQuery {
    since: Option<String>,
    limit: Option<i64>,
}

async fn changes(
    State(state): State<DriveState>,
    headers: HeaderMap,
    Query(query): Query<ChangesQuery>,
) -> DocumentsResult<Json<DriveChanges>> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    let since = query
        .since
        .as_deref()
        .filter(|since| !since.is_empty())
        .map(parse_drive_cursor)
        .transpose()?;
    let limit = query.limit.unwrap_or(DEFAULT_CHANGES);
    Ok(Json(
        act_as(
            actor,
            state.service.drive_changes(&who, since, limit, &trace),
        )
        .await?,
    ))
}

async fn blob(
    State(state): State<DriveState>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> DocumentsResult<Response> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    let hash = ContentHash::try_new(sha256).map_err(ApiError::from)?;
    let tag = etag(&hash);
    if none_match(&headers, &tag) {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let read = act_as(
        actor,
        state
            .service
            .drive_blob(&who, &hash, range(&headers), &trace),
    )
    .await?;
    let status = if read.range.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let content_range = read.range.map(|served| served.content_range());
    let mut response = Response::new(Body::from_stream(read.body));
    *response.status_mut() = status;
    let out = response.headers_mut();
    out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(value) = HeaderValue::from_str(&tag) {
        out.insert(header::ETAG, value);
    }
    if let Some(value) = content_range.and_then(|value| HeaderValue::from_str(&value).ok()) {
        out.insert(header::CONTENT_RANGE, value);
    }
    Ok(response)
}

async fn commit(
    State(state): State<DriveState>,
    headers: HeaderMap,
    form: Multipart,
) -> DocumentsResult<Response> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    let (mut meta, bytes) = read_commit(form).await?;
    if let Some(base) = base_revision(&headers)? {
        if meta
            .base_revision
            .as_ref()
            .is_some_and(|declared| declared != &base)
        {
            return Err(ApiError::bad_request(
                "If-Match and meta.base_revision name different revisions",
            )
            .into());
        }
        meta.base_revision = Some(base);
    }
    let outcome = Box::pin(act_as(
        actor,
        state.service.drive_commit(&who, meta, bytes, &trace),
    ))
    .await?;
    let status = match outcome {
        DriveCommitResponse::Committed { .. } | DriveCommitResponse::Merged { .. } => {
            StatusCode::OK
        },
        DriveCommitResponse::Conflict { .. } => StatusCode::CONFLICT,
        DriveCommitResponse::Proposed { .. } => StatusCode::CREATED,
    };
    Ok((status, Json(outcome)).into_response())
}

async fn read_commit(mut form: Multipart) -> DocumentsResult<(DriveCommitMeta, Vec<u8>)> {
    let mut meta = None;
    let mut blob = None;
    while let Some(field) = form.next_field().await? {
        let part = field.name().unwrap_or_default().to_owned();
        let bytes = field.bytes().await?;
        match part.as_str() {
            "meta" => {
                meta = Some(serde_json::from_slice::<DriveCommitMeta>(&bytes).map_err(
                    |source| {
                        ApiError::bad_request("meta is not a drive commit").with_source(source)
                    },
                )?);
            },
            "blob" => blob = Some(bytes.to_vec()),
            _ => {},
        }
    }
    let meta = meta.ok_or_else(|| ApiError::bad_request("commit needs a `meta` part"))?;
    let blob = blob.ok_or_else(|| ApiError::bad_request("commit needs a `blob` part"))?;
    Ok((meta, blob))
}

async fn presence(
    State(state): State<DriveState>,
    headers: HeaderMap,
    Json(beacon): Json<PresenceBeacon>,
) -> DocumentsResult<StatusCode> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    act_as(actor, state.service.touch_presence(&who, beacon, &trace)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn proposals(
    State(state): State<DriveState>,
    headers: HeaderMap,
) -> DocumentsResult<Json<DriveProposals>> {
    let (who, actor) = device_user(&state, &headers).await?;
    let proposals = act_as(actor, state.service.open_proposals_for(&who)).await?;
    Ok(Json(DriveProposals {
        proposals: proposals
            .into_iter()
            .map(|p| DriveProposal {
                proposal_id: p.id,
                document_id: p.document_id,
                proposer_id: p.proposer_user_id,
                actor_kind: p.actor_kind,
                message: p.message,
                created_at: p.created_at,
            })
            .collect(),
    }))
}
