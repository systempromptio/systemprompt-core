use std::time::Duration;

use serde_json::json;
use sp_client::ClientError;
use sp_drive::{ConflictHead, DriveError, NeededCapability};

use crate::support::{DOC, FakeInstance, FakeResponse, document_id};

const REV: &str = "11111111-2222-4333-8444-555555555555";
const BASE: &str = "22222222-3333-4444-8555-666666666666";
const SHA: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

async fn answer(response: FakeResponse) -> DriveError {
    let fake = FakeInstance::start(move |_| response.clone()).await;
    fake.drive().document(&document_id(DOC)).await.unwrap_err()
}

async fn answer_json(status: u16, body: serde_json::Value) -> DriveError {
    answer(FakeResponse::json(status, &body)).await
}

#[tokio::test]
async fn not_found_and_forbidden() {
    let missing = answer_json(404, json!({ "error_key": "not_found" })).await;
    assert!(
        matches!(missing, DriveError::NotFound { error_key: Some(ref key), .. } if key == "not_found")
    );
    let no_route = answer(FakeResponse::bytes(404, b"<html>no route</html>".to_vec())).await;
    assert!(matches!(
        no_route,
        DriveError::NotFound {
            error_key: None,
            ..
        }
    ));
    let forbidden = answer_json(403, json!({ "error_key": "forbidden" })).await;
    assert!(
        matches!(forbidden, DriveError::Forbidden { error_key: Some(ref key), .. } if key == "forbidden")
    );
}

#[tokio::test]
async fn agent_refusals_name_the_missing_capability() {
    let suggest = answer_json(
        403,
        json!({ "error_key": "agent_suggest_only", "message": "nope" }),
    )
    .await;
    assert!(matches!(
        suggest,
        DriveError::AgentRefused {
            capability: NeededCapability::Edit,
            ..
        }
    ));
    let person = answer_json(403, json!({ "error_key": "agent_forbidden" })).await;
    assert!(matches!(
        person,
        DriveError::AgentRefused {
            capability: NeededCapability::Person,
            ..
        }
    ));
    let other = answer_json(403, json!({ "error_key": "agent_capped" })).await;
    assert!(matches!(
        other,
        DriveError::AgentRefused {
            capability: NeededCapability::Other,
            ..
        }
    ));
    assert_eq!(other.error_key(), Some("agent_capped"));
}

#[tokio::test]
async fn conflicts_carry_the_head_from_either_place() {
    let top = answer_json(
        409,
        json!({ "error_key": "conflict", "head_revision_id": REV, "head_content_sha256": SHA, "base_revision_id": BASE }),
    )
    .await;
    let head = top.conflict_head().unwrap();
    assert_eq!(
        head.head_revision_id
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some(REV)
    );
    assert_eq!(head.head_content_sha256.as_deref(), Some(SHA));
    assert_eq!(
        head.base_revision_id
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some(BASE)
    );
    let nested = answer_json(
        409,
        json!({ "error_key": "conflict", "detail": { "head_revision_id": REV } }),
    )
    .await;
    let nested_head = nested
        .conflict_head()
        .and_then(|head| head.head_revision_id.as_ref())
        .map(ToString::to_string);
    assert_eq!(nested_head.as_deref(), Some(REV));
    let bare = answer_json(409, json!({ "error_key": "missing_parts" })).await;
    assert_eq!(bare.conflict_head(), Some(&ConflictHead::default()));
    assert_eq!(bare.error_key(), Some("missing_parts"));
}

#[tokio::test]
async fn client_statuses_keep_the_client_mapping() {
    let refused = answer_json(401, json!({})).await;
    assert!(matches!(
        refused,
        DriveError::Client(ClientError::Unauthorized { .. })
    ));
    let mismatch = answer_json(401, json!({ "error_key": "session_mismatch" })).await;
    assert!(matches!(
        mismatch,
        DriveError::Client(ClientError::SessionMismatch { .. })
    ));
    let limited = answer(FakeResponse::bytes(429, Vec::new()).header("Retry-After", "4")).await;
    assert!(limited.is_transient());
    assert!(matches!(
        limited,
        DriveError::Client(ClientError::RateLimited { retry_after: Some(delay), .. }) if delay == Duration::from_secs(4)
    ));
    let down = answer_json(503, json!({})).await;
    assert!(matches!(
        down,
        DriveError::Client(ClientError::Unreachable { .. })
    ));
    let odd = answer_json(422, json!({})).await;
    assert!(matches!(
        odd,
        DriveError::Client(ClientError::Protocol {
            status: Some(422),
            ..
        })
    ));
}

#[tokio::test]
async fn error_keys_are_plain_and_messages_dropped() {
    let error = answer_json(
        403,
        json!({ "error_key": "has space", "message": "token sp-live-abc" }),
    )
    .await;
    assert!(matches!(
        error,
        DriveError::Forbidden {
            error_key: None,
            ..
        }
    ));
    assert!(!error.to_string().contains("sp-live"));
}
