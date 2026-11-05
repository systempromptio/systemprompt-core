//! Local deletes reach the server only when they are certainly meant.

use chrono::{Duration, Utc};
use systemprompt_bridge::drive::deletes::{HeldChoice, HeldReason, apply, observe};
use systemprompt_bridge::drive::pull::pull;
use systemprompt_bridge::drive::state::DriveState;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{Fixture, manifest_json};

const REV: &str = "019d0000-0000-7000-8000-00000000000a";
const HEAD: &str = "019d0000-0000-7000-8000-00000000000b";

fn doc(n: u32) -> DocumentId {
    DocumentId::new(format!("019d0000-0000-7000-8000-{n:012}"))
}

fn rev() -> DocumentRevisionId {
    DocumentRevisionId::new(REV)
}

fn seed(fx: &Fixture, n: u32) -> String {
    let rel = format!("f{n}.md");
    fx.write(&rel, format!("body {n}").as_bytes());
    fx.synced(&rel, &doc(n), &rev());
    rel
}

fn remove(fx: &Fixture, rel: &str) {
    std::fs::remove_file(fx.root.dir().join(rel)).expect("remove");
}

fn observed_long_ago(fx: &Fixture) -> DriveState {
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now() - Duration::seconds(60));
    observe(&fx.root, &mut state, Utc::now());
    state
}

async fn expect_trash(server: &MockServer, times: u64, status: u16, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_TRASH))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .expect(times)
        .mount(server)
        .await;
}

async fn serve_head(server: &MockServer, n: u32, bytes: &[u8]) {
    let rel = format!("f{n}.md");
    let head = DocumentRevisionId::new(HEAD);
    let id = doc(n);
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(&[(
            &id,
            rel.as_str(),
            &head,
            bytes,
        )])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(bytes))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn one_cycle_of_absence_trashes_nothing() {
    let server = MockServer::start().await;
    expect_trash(&server, 0, 200, serde_json::json!({"status": "trashed"})).await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now() - Duration::seconds(60));
    apply(fx.cycle(), &mut state).await.expect("apply");
    assert!(state.entries.contains_key(&rel));
}

#[tokio::test]
async fn two_quick_cycles_are_not_enough() {
    let server = MockServer::start().await;
    expect_trash(&server, 0, 200, serde_json::json!({"status": "trashed"})).await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now());
    observe(&fx.root, &mut state, Utc::now());
    apply(fx.cycle(), &mut state).await.expect("apply");
    assert!(state.entries.contains_key(&rel));
}

#[tokio::test]
async fn an_editor_save_that_recreates_the_file_is_not_a_delete() {
    let server = MockServer::start().await;
    expect_trash(&server, 0, 200, serde_json::json!({"status": "trashed"})).await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now() - Duration::seconds(60));
    fx.write(&rel, b"saved again");
    observe(&fx.root, &mut state, Utc::now());
    apply(fx.cycle(), &mut state).await.expect("apply");
    assert!(state.pending_deletes.is_empty());
    assert!(state.entries.contains_key(&rel));
}

#[tokio::test]
async fn a_confirmed_absence_trashes_with_if_match_on_the_synced_revision() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_TRASH))
        .and(header("if-match", format!("\"{REV}\"").as_str()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "trashed"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = observed_long_ago(&fx);
    let report = apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(report.trashed, 1);
    assert!(!state.entries.contains_key(&rel));
    assert!(state.pending_deletes.is_empty());
}

#[tokio::test]
async fn the_same_bytes_under_a_new_name_are_a_move() {
    let server = MockServer::start().await;
    expect_trash(&server, 0, 200, serde_json::json!({"status": "trashed"})).await;
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MOVE))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    std::fs::rename(fx.root.dir().join(&rel), fx.root.dir().join("renamed.md")).expect("rename");
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now());
    let report = apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(report.moved, 1);
    let entry = state.entries.get("renamed.md").expect("moved entry");
    assert_eq!(entry.document_id, Some(doc(1)));
    assert!(!state.entries.contains_key(&rel));
}

#[tokio::test]
async fn a_refused_move_puts_the_file_back() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MOVE))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    std::fs::rename(fx.root.dir().join(&rel), fx.root.dir().join("renamed.md")).expect("rename");
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now());
    apply(fx.cycle(), &mut state).await.expect("apply");
    assert!(fx.root.dir().join(&rel).exists());
    assert!(!fx.root.dir().join("renamed.md").exists());
    assert_eq!(state.delete_notices.len(), 1);
}

#[tokio::test]
async fn an_emptied_folder_holds_every_delete() {
    let server = MockServer::start().await;
    expect_trash(&server, 0, 200, serde_json::json!({"status": "trashed"})).await;
    let fx = Fixture::new(&server.uri());
    let rels = [seed(&fx, 1), seed(&fx, 2)];
    for rel in &rels {
        remove(&fx, rel);
    }
    let mut state = observed_long_ago(&fx);
    let report = apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(report.held.expect("held").reason, HeldReason::RootEmpty);
    assert_eq!(state.entries.len(), 2);
}

#[tokio::test]
async fn a_bulk_absence_holds_until_the_user_applies_it() {
    let server = MockServer::start().await;
    expect_trash(&server, 11, 200, serde_json::json!({"status": "trashed"})).await;
    let fx = Fixture::new(&server.uri());
    let rels: Vec<String> = (1..=30).map(|n| seed(&fx, n)).collect();
    for rel in &rels[..11] {
        remove(&fx, rel);
    }
    let mut state = observed_long_ago(&fx);
    let report = apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(report.held.expect("held").reason, HeldReason::Bulk);
    assert_eq!(state.entries.len(), 30);
    state.held_choice = Some(HeldChoice::Apply);
    let report = apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(report.trashed, 11);
    assert!(state.held_deletes.is_none());
}

#[tokio::test]
async fn a_held_delete_can_be_restored() {
    let server = MockServer::start().await;
    expect_trash(&server, 0, 200, serde_json::json!({"status": "trashed"})).await;
    serve_head(&server, 1, b"body 1").await;
    let fx = Fixture::new(&server.uri());
    let first = seed(&fx, 1);
    let second = seed(&fx, 2);
    remove(&fx, &first);
    remove(&fx, &second);
    let mut state = observed_long_ago(&fx);
    apply(fx.cycle(), &mut state).await.expect("hold");
    state.held_choice = Some(HeldChoice::Restore);
    apply(fx.cycle(), &mut state).await.expect("restore");
    assert_eq!(fx.read(&first), b"body 1");
    assert!(state.pending_deletes.is_empty());
}

#[tokio::test]
async fn a_newer_head_restores_the_file_with_a_notice() {
    let server = MockServer::start().await;
    expect_trash(
        &server,
        1,
        409,
        serde_json::json!({
            "status": "conflict", "head_revision_id": HEAD,
            "head_content_sha256": content_hash(b"newer"),
        }),
    )
    .await;
    serve_head(&server, 1, b"newer").await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = observed_long_ago(&fx);
    let report = apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(report.restored, 1);
    assert_eq!(fx.read(&rel), b"newer");
    assert_eq!(state.delete_notices.len(), 1);
}

#[tokio::test]
async fn a_refused_delete_restores_the_file_with_a_notice() {
    let server = MockServer::start().await;
    expect_trash(&server, 1, 403, serde_json::json!({"message": "not yours"})).await;
    serve_head(&server, 1, b"body 1").await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = observed_long_ago(&fx);
    apply(fx.cycle(), &mut state).await.expect("apply");
    assert_eq!(fx.read(&rel), b"body 1");
    assert_eq!(state.delete_notices.len(), 1);
    assert!(state.pending_deletes.is_empty());
}

#[tokio::test]
async fn a_server_failure_keeps_the_delete_pending() {
    let server = MockServer::start().await;
    expect_trash(&server, 1, 503, serde_json::json!({})).await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    seed(&fx, 2);
    remove(&fx, &rel);
    let mut state = observed_long_ago(&fx);
    assert!(apply(fx.cycle(), &mut state).await.is_err());
    assert!(state.pending_deletes.contains_key(&rel));
}

#[tokio::test]
async fn the_pull_does_not_bring_back_a_pending_delete() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "next_cursor": "c9",
            "changes": [{
                "cursor": "c9", "document_id": doc(1), "path": "f1.md",
                "revision_id": HEAD, "content_sha256": content_hash(b"newer"),
                "kind": {"type": "updated"},
            }],
        })))
        .mount(&server)
        .await;
    let fx = Fixture::new(&server.uri());
    let rel = seed(&fx, 1);
    remove(&fx, &rel);
    let mut state = fx.state();
    observe(&fx.root, &mut state, Utc::now());
    pull(fx.cycle(), &mut state).await.expect("pull");
    assert!(!fx.root.dir().join(&rel).exists());
    assert_eq!(state.entries.get(&rel).expect("entry").revision_id, rev());
}
