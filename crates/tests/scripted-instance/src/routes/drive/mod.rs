//! The Drive sync feed over an in-memory store: the sync token, the
//! manifest, the change feed, blobs, commits, trash and moves.
//!
//! The feed takes its own credential. The device token a sign-in returns
//! beside the session token is traded for a sync token, and every other
//! route accepts only that. A commit or a trash whose base is not the head
//! answers a conflict naming the head and is not applied; nothing is merged.

mod listing;
mod store;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path as FilePath;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header::{CONTENT_TYPE, IF_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{TimeDelta, Utc};
use serde_json::{Map, json};
use systemprompt_identifiers::ContentHash;
use systemprompt_models::documents::{
    DRIVE_SYNC_AUDIENCE, DocumentFormat, DriveChangeKind, DriveChanges, DriveCommitMeta,
    DriveCommitResponse, DriveManifest, DriveMoveRequest, DriveSyncToken, DriveTrashRequest,
    DriveTrashResponse, content_hash,
};

pub(crate) use self::listing::{documents, folders};
pub(crate) use self::store::DriveStore;
use super::{bearer, refuse, refuse_as, unauthorized};
use crate::state::{InstanceState, lock};
use crate::token::{DEVICE_TOKEN_USE, SYNC_USE, TOKEN_TTL_SECS};

const OCTET_STREAM: &str = "application/octet-stream";
const CURSOR_RESET: &str = "cursor_reset";
const META_PART: &str = "meta";
const BLOB_PART: &str = "blob";
const SINCE: &str = "since";

fn holds(state: &InstanceState, headers: &HeaderMap, token_use: &str) -> bool {
    bearer(headers).is_some_and(|token| state.keys.claims(token_use, token).is_some())
}

fn syncing(state: &InstanceState, headers: &HeaderMap) -> Result<(), Response> {
    if holds(state, headers, SYNC_USE) {
        Ok(())
    } else {
        Err(unauthorized(
            "a Drive sync token this instance issued is required",
        ))
    }
}

pub(crate) async fn token(State(state): State<Arc<InstanceState>>, headers: HeaderMap) -> Response {
    if !holds(&state, &headers, DEVICE_TOKEN_USE) {
        return unauthorized("only the device token trades for a Drive sync token");
    }
    let mut claims = Map::new();
    claims.insert("sub".to_owned(), json!(state.user.user_id));
    claims.insert("aud".to_owned(), json!(DRIVE_SYNC_AUDIENCE));
    Json(DriveSyncToken {
        token: state.keys.mint(SYNC_USE, claims),
        expires_at: Utc::now() + TimeDelta::seconds(TOKEN_TTL_SECS),
    })
    .into_response()
}

pub(crate) async fn manifest(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = syncing(&state, &headers) {
        return refused;
    }
    let drive = lock(&state.drive);
    Json(DriveManifest {
        generated_at: Utc::now(),
        cursor: drive.cursor(),
        entries: drive.documents.values().cloned().collect(),
    })
    .into_response()
}

pub(crate) async fn changes(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    if let Err(refused) = syncing(&state, &headers) {
        return refused;
    }
    let drive = lock(&state.drive);
    let after = query
        .get(SINCE)
        .and_then(|since| since.parse::<usize>().ok())
        .and_then(|since| drive.changes.get(since..));
    match after {
        Some(after) => Json(DriveChanges {
            changes: after.to_vec(),
            next_cursor: drive.cursor(),
        })
        .into_response(),
        None => refuse_as(
            StatusCode::GONE,
            Some(CURSOR_RESET),
            "the change feed does not know this cursor",
        ),
    }
}

pub(crate) async fn blob(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Response {
    if let Err(refused) = syncing(&state, &headers) {
        return refused;
    }
    let held = ContentHash::try_new(sha256)
        .ok()
        .and_then(|address| lock(&state.drive).blobs.get(&address).cloned());
    match held {
        Some(bytes) => ([(CONTENT_TYPE, OCTET_STREAM)], bytes).into_response(),
        None => not_found(),
    }
}

pub(crate) async fn commit(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    mut parts: Multipart,
) -> Response {
    if let Err(refused) = syncing(&state, &headers) {
        return refused;
    }
    let mut meta = None;
    let mut bytes = None;
    while let Ok(Some(part)) = parts.next_field().await {
        match part.name() {
            Some(META_PART) => {
                meta = part
                    .text()
                    .await
                    .ok()
                    .and_then(|text| serde_json::from_str::<DriveCommitMeta>(&text).ok());
            },
            Some(BLOB_PART) => bytes = part.bytes().await.ok().map(|bytes| bytes.to_vec()),
            _ => {},
        }
    }
    let (Some(meta), Some(bytes)) = (meta, bytes) else {
        return invalid("a commit carries a meta part and a blob part");
    };
    if content_hash(&bytes) != meta.content_sha256 {
        return invalid("the blob does not match the digest its meta names");
    }
    let mut drive = lock(&state.drive);
    let held = drive.held(meta.document_id.as_ref(), &meta.path);
    if let Some(head) = &held
        && meta.base_revision.as_ref() != Some(&head.head_revision_id)
    {
        let outcome = DriveCommitResponse::Conflict {
            head_revision_id: head.head_revision_id.clone(),
            head_content_sha256: head.content_sha256.clone(),
            base_revision_id: meta.base_revision,
        };
        return (StatusCode::CONFLICT, Json(outcome)).into_response();
    }
    let revision_id = drive.write(meta.document_id, meta.path, bytes);
    drop(drive);
    Json(DriveCommitResponse::Committed { revision_id }).into_response()
}

pub(crate) async fn trash(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Json(request): Json<DriveTrashRequest>,
) -> Response {
    if let Err(refused) = syncing(&state, &headers) {
        return refused;
    }
    let base = headers
        .get(IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().trim_matches('"'));
    let mut drive = lock(&state.drive);
    let Some(head) = drive.documents.get(&request.document_id).cloned() else {
        return not_found();
    };
    if base != Some(head.head_revision_id.as_str()) {
        let outcome = DriveTrashResponse::Conflict {
            head_revision_id: head.head_revision_id,
            head_content_sha256: head.content_sha256,
        };
        return (StatusCode::CONFLICT, Json(outcome)).into_response();
    }
    drive.documents.remove(&request.document_id);
    drive.note(&head, DriveChangeKind::Deleted);
    drop(drive);
    Json(DriveTrashResponse::Trashed).into_response()
}

pub(crate) async fn move_document(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Json(request): Json<DriveMoveRequest>,
) -> Response {
    if let Err(refused) = syncing(&state, &headers) {
        return refused;
    }
    let mut drive = lock(&state.drive);
    let Some(entry) = drive.documents.get_mut(&request.document_id) else {
        return not_found();
    };
    let from = std::mem::replace(&mut entry.path, request.path);
    let moved = entry.clone();
    drive.note(&moved, DriveChangeKind::Renamed { from });
    drop(drive);
    StatusCode::NO_CONTENT.into_response()
}

fn format_of(path: &str) -> DocumentFormat {
    FilePath::new(path)
        .extension()
        .and_then(OsStr::to_str)
        .and_then(DocumentFormat::from_extension)
        .unwrap_or(DocumentFormat::File)
}

fn not_found() -> Response {
    refuse(
        StatusCode::NOT_FOUND,
        "the in-memory Drive holds no such document or blob",
    )
}

fn invalid(message: &str) -> Response {
    refuse(StatusCode::UNPROCESSABLE_ENTITY, message)
}
