use systemprompt_bridge::drive::pull::pull;
use systemprompt_bridge::drive::run::{ensure_enabled, sync_once};
use systemprompt_bridge::drive::{DriveError, DriveHandle};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{Fixture, manifest_json};

async fn serve_manifest(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn serve_blob(server: &MockServer, bytes: &[u8]) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(bytes))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
        .mount(server)
        .await;
}

async fn serve_no_proposals(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_PROPOSALS))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"proposals": []})),
        )
        .mount(server)
        .await;
}

async fn serve_changes(server: &MockServer, changes: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": changes, "next_cursor": "c9",
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_first_cycle_reconciles_the_manifest_and_records_a_clean_status() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    let body: &[u8] = b"# from the web";
    serve_manifest(
        &server,
        manifest_json(&[(&doc, "notes/web.md", &rev, body)]),
    )
    .await;
    serve_blob(&server, body).await;
    serve_no_proposals(&server).await;
    let handle = DriveHandle::new();
    handle.update(|s| s.last_error = Some("stale".to_owned()));

    let report = sync_once(fx.cycle(), &handle).await.expect("cycle");

    assert_eq!(report.pull.applied, 1);
    assert_eq!(report.pending_uploads, 0);
    assert_eq!(fx.read("notes/web.md"), body);
    let status = handle.status();
    assert!(status.enabled);
    assert_eq!(status.root.as_deref(), Some(fx.root.dir()));
    assert!(status.synced_at.is_some());
    assert_eq!(status.last_error, None);
    assert_eq!(status.pending_uploads, 0);
    let state = fx.state();
    assert_eq!(state.cursor.as_deref(), Some("c-manifest"));
    assert_eq!(state.entries["notes/web.md"].revision_id, rev);
}

#[tokio::test]
async fn a_failed_pull_leaves_the_status_unsynced() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let handle = DriveHandle::new();

    sync_once(fx.cycle(), &handle)
        .await
        .expect_err("500 fails the cycle");

    assert!(handle.status().synced_at.is_none());
    assert!(!handle.status().enabled);
}

#[tokio::test]
async fn a_missing_folder_is_created_once_the_manifest_lists_documents() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    std::fs::remove_dir_all(fx.root.dir()).expect("remove");
    let doc = DocumentId::generate();
    serve_manifest(
        &server,
        manifest_json(&[(&doc, "a.md", &DocumentRevisionId::generate(), b"a")]),
    )
    .await;
    let handle = DriveHandle::new();

    assert!(ensure_enabled(fx.cycle(), &handle).await.expect("enabled"));

    assert!(fx.root.dir().is_dir());
    let status = handle.status();
    assert!(status.enabled);
    assert_eq!(status.root.as_deref(), Some(fx.root.dir()));
}

#[tokio::test]
async fn an_empty_manifest_leaves_the_folder_absent() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    std::fs::remove_dir_all(fx.root.dir()).expect("remove");
    serve_manifest(&server, manifest_json(&[])).await;
    let handle = DriveHandle::new();

    assert!(!ensure_enabled(fx.cycle(), &handle).await.expect("answered"));

    assert!(!fx.root.dir().exists());
    assert!(!handle.status().enabled);
}

#[tokio::test]
async fn a_manifest_server_error_is_returned_not_swallowed() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    std::fs::remove_dir_all(fx.root.dir()).expect("remove");
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let handle = DriveHandle::new();

    let err = ensure_enabled(fx.cycle(), &handle).await.expect_err("503");

    assert!(!matches!(err, DriveError::ServerDisabled { .. }), "{err:?}");
    assert!(!fx.root.dir().exists());
}

#[tokio::test]
async fn a_remote_rename_moves_the_file_and_rekeys_its_entry() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    fx.write("old.md", b"same bytes");
    fx.synced("old.md", &doc, &rev);
    serve_changes(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "moved/new.md",
            "kind": {"type": "renamed", "from": "old.md"},
        }]),
    )
    .await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    assert!(report.conflicts.is_empty());
    assert!(!fx.root.dir().join("old.md").exists());
    assert_eq!(fx.read("moved/new.md"), b"same bytes");
    let saved = fx.state();
    assert!(!saved.entries.contains_key("old.md"));
    assert_eq!(saved.entries["moved/new.md"].revision_id, rev);
    assert_eq!(saved.cursor.as_deref(), Some("c9"));
}

#[tokio::test]
async fn a_rename_carrying_a_new_revision_writes_the_new_head() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("old.md", b"before");
    fx.synced("old.md", &doc, &DocumentRevisionId::generate());
    let body: &[u8] = b"after the rename";
    let rev = DocumentRevisionId::generate();
    serve_changes(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "new.md",
            "revision_id": rev.as_str(), "content_sha256": content_hash(body).as_str(),
            "kind": {"type": "renamed", "from": "old.md"},
        }]),
    )
    .await;
    serve_blob(&server, body).await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(report.applied, 1);
    assert_eq!(fx.read("new.md"), body);
    assert_eq!(fx.state().entries["new.md"].revision_id, rev);
}

#[tokio::test]
async fn an_update_without_a_revision_is_refused_as_incomplete() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    serve_changes(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": DocumentId::generate().as_str(), "path": "a.md",
            "kind": {"type": "updated"},
        }]),
    )
    .await;

    let mut state = fx.state();
    let err = pull(fx.cycle(), &mut state).await.expect_err("incomplete");

    assert!(
        matches!(err, DriveError::IncompleteChange { .. }),
        "{err:?}"
    );
    assert_eq!(fx.state().cursor.as_deref(), Some("c0"));
}

#[tokio::test]
async fn a_local_file_already_at_the_head_is_adopted_without_a_download() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    let body: &[u8] = b"identical";
    fx.write("a.md", body);
    serve_manifest(&server, manifest_json(&[(&doc, "a.md", &rev, body)])).await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(report.applied, 1);
    assert_eq!(fx.state().entries["a.md"].revision_id, rev);
    assert!(
        server
            .received_requests()
            .await
            .expect("recorded")
            .iter()
            .all(|r| !r.url.path().starts_with(ApiPaths::BRIDGE_DRIVE_BLOB)),
        "no blob is fetched"
    );

    let manifest =
        serde_json::from_value(manifest_json(&[(&doc, "a.md", &rev, body)])).expect("manifest");
    let mut state = fx.state();
    let again = systemprompt_bridge::drive::pull::reconcile(fx.cycle(), &mut state, manifest)
        .await
        .expect("reconcile");
    assert_eq!(again.applied, 0, "an entry already at the head is skipped");
}

#[tokio::test]
async fn an_unsynced_local_edit_is_kept_beside_the_incoming_head() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    let theirs: &[u8] = b"their version";
    fx.write("a.md", b"my unsynced version");
    serve_manifest(&server, manifest_json(&[(&doc, "a.md", &rev, theirs)])).await;
    serve_blob(&server, theirs).await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(report.conflicts.len(), 1);
    let notice = &report.conflicts[0];
    assert_eq!(notice.path, "a.md");
    assert_eq!(notice.head_revision_id, rev);
    assert_eq!(fx.read("a.md"), theirs);
    assert_eq!(fx.read(&notice.sibling), b"my unsynced version");
}
