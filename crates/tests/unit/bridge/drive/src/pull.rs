use systemprompt_bridge::drive::DriveError;
use systemprompt_bridge::drive::pull::pull;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{Fixture, manifest_json};

async fn feed(server: &MockServer, changes: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .and(query_param("since", "c0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": changes, "next_cursor": "c9",
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn pull_rejects_a_blob_whose_digest_mismatches() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("a.md", b"local original");
    fx.synced("a.md", &doc, &DocumentRevisionId::generate());
    let promised = content_hash(b"the real update");

    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "a.md",
            "revision_id": DocumentRevisionId::generate().as_str(),
            "content_sha256": promised.as_str(), "kind": {"type": "updated"},
        }]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&promised)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tampered".to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    let err = pull(fx.cycle(), &mut state)
        .await
        .expect_err("mismatch must fail");
    assert!(matches!(err, DriveError::HashMismatch { .. }), "{err:?}");
    assert_eq!(fx.read("a.md"), b"local original");
    assert_eq!(
        fx.state().cursor.as_deref(),
        Some("c0"),
        "cursor does not advance"
    );
}

#[tokio::test]
async fn pull_writes_a_verified_update() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    fx.write("a.md", b"old");
    fx.synced("a.md", &doc, &DocumentRevisionId::generate());
    let body: &[u8] = b"new from the web";

    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "a.md",
            "revision_id": rev.as_str(), "content_sha256": content_hash(body).as_str(),
            "kind": {"type": "updated"},
        }]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(body))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");
    assert_eq!(fx.read("a.md"), body);
    let saved = fx.state();
    assert_eq!(saved.cursor.as_deref(), Some("c9"));
    assert_eq!(saved.entries["a.md"].revision_id, rev);
}

#[tokio::test]
async fn deleted_document_moves_to_the_trash() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("old/notes.md", b"keep me somewhere");
    fx.synced("old/notes.md", &doc, &DocumentRevisionId::generate());

    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "old/notes.md",
            "kind": {"type": "deleted"},
        }]),
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert!(!fx.root.dir().join("old/notes.md").exists());
    let trash = fx.root.trash_dir();
    let stamp = std::fs::read_dir(&trash)
        .expect("trash")
        .next()
        .expect("one stamp")
        .expect("entry");
    assert_eq!(
        std::fs::read(stamp.path().join("old/notes.md")).expect("trashed"),
        b"keep me somewhere"
    );
    assert!(!fx.state().entries.contains_key("old/notes.md"));
}

#[tokio::test]
async fn pull_refuses_a_path_outside_the_drive_folder() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    let body: &[u8] = b"escape";
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": DocumentId::generate().as_str(), "path": "../escape.md",
            "revision_id": DocumentRevisionId::generate().as_str(),
            "content_sha256": content_hash(body).as_str(), "kind": {"type": "created"},
        }]),
    )
    .await;

    let mut state = fx.state();
    let err = pull(fx.cycle(), &mut state).await.expect_err("unsafe path");
    assert!(matches!(err, DriveError::UnsafePath(_)), "{err:?}");
}

#[tokio::test]
async fn a_cursor_reset_drops_the_cursor_and_reconciles_from_the_manifest() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    let body: &[u8] = b"from the manifest";

    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .and(query_param("since", "c0"))
        .respond_with(ResponseTemplate::new(410).set_body_json(serde_json::json!({
            "code": "gone", "error_key": "cursor_reset",
            "message": "drive cursor \"c0\" is not a feed position; restart from the manifest",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(manifest_json(&[(&doc, "a.md", &rev, body)])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(body))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state)
        .await
        .expect("pull after reset");
    assert_eq!(fx.read("a.md"), body);
    assert_eq!(fx.state().cursor.as_deref(), Some("c-manifest"));
}

#[tokio::test]
async fn a_gone_answer_without_the_reset_key_is_an_error() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .respond_with(ResponseTemplate::new(410).set_body_json(serde_json::json!({
            "code": "gone", "message": "gone",
        })))
        .mount(&server)
        .await;

    let mut state = fx.state();
    let err = pull(fx.cycle(), &mut state).await.expect_err("not a reset");
    assert!(matches!(err, DriveError::Rejected { .. }), "{err:?}");
    assert_eq!(fx.state().cursor.as_deref(), Some("c0"));
}
