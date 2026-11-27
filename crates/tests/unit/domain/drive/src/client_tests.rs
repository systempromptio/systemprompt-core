use serde_json::json;
use systemprompt_device::client::ClientError;
use systemprompt_drive::DriveError;
use systemprompt_identifiers::ContextId;
use systemprompt_models::documents::client::CreateDocumentRequest;

use crate::support::{FakeInstance, FakeResponse, sha256};

const CONTEXT: &str = "5d1e7c2a-8b3f-4a6d-9e0c-1f2a3b4c5d6e";

fn context() -> ContextId {
    ContextId::try_new(CONTEXT).unwrap()
}

#[tokio::test]
async fn a_context_is_renamed_on_its_own_route() {
    let fake = FakeInstance::start(|_| FakeResponse::json(200, &json!({}))).await;
    fake.drive()
        .rename_context(&context(), "Notes")
        .await
        .unwrap();
    let sent = fake.last();
    assert_eq!(sent.method, "PUT");
    assert_eq!(sent.path(), format!("/api/v1/core/contexts/{CONTEXT}"));
    assert_eq!(sent.json(), json!({ "name": "Notes" }));
    assert_eq!(
        sent.header("authorization"),
        Some("Bearer header.claims.signature")
    );
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_created_context_answers_its_typed_id() {
    let fake = FakeInstance::start(|_| {
        FakeResponse::json(
            201,
            &json!({ "data": { "context_id": CONTEXT, "name": "Notes" } }),
        )
    })
    .await;
    assert_eq!(
        fake.drive().create_context("Notes").await.unwrap(),
        context()
    );
    let malformed = FakeInstance::start(|_| {
        FakeResponse::json(
            201,
            &json!({ "data": { "context_id": "ctx-1", "name": "Notes" } }),
        )
    })
    .await;
    assert!(malformed.drive().create_context("Notes").await.is_err());
}

#[tokio::test]
async fn a_write_names_its_context_in_a_header() {
    let fake = FakeInstance::start(|_| FakeResponse::json(201, &json!({}))).await;
    let request = CreateDocumentRequest {
        template_id: "blank".to_owned(),
        name: "Notes".to_owned(),
        folder_id: None,
    };
    let undecodable = fake
        .drive()
        .create_document(&request, Some(&context()))
        .await;
    assert!(undecodable.is_err());
    assert_eq!(fake.last().header("x-context-id"), Some(CONTEXT));
    assert!(fake.drive().create_document(&request, None).await.is_err());
    assert_eq!(fake.last().header("x-context-id"), None);
}

#[tokio::test]
async fn a_refused_token_is_minted_again_once() {
    let fake = FakeInstance::start(|_| FakeResponse::json(401, &json!({}))).await;
    let refused = fake
        .drive()
        .rename_context(&context(), "Notes")
        .await
        .unwrap_err();
    assert!(matches!(
        refused,
        DriveError::Client(ClientError::Unauthorized { .. })
    ));
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn an_upload_opens_with_the_digest_and_size_of_its_bytes() {
    let fake = FakeInstance::start(|_| {
        FakeResponse::json(
            201,
            &json!({
                "upload_id": "0b1c2d3e-4f50-4617-8293-a4b5c6d7e8f9",
                "name": "a.txt", "mime": "text/plain", "size": 3,
                "sha256": sha256(b"abc"), "part_size": 8, "part_count": 1, "status": "open",
                "missing_parts": [1], "expires_at": "2026-11-26T00:00:00Z"
            }),
        )
    })
    .await;
    let opened = fake
        .drive()
        .open_upload("a.txt", "text/plain", b"abc")
        .await
        .unwrap();
    assert_eq!(opened.missing_parts, vec![1]);
    let body = fake.last().json();
    assert_eq!(
        body["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(body["size"], 3);
    assert_eq!(body["name"], "a.txt");
}
