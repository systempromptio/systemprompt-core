use systemprompt_bridge::drive::pull::pull;
use systemprompt_bridge::drive::{ConflictNotice, DriveState};
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

async fn serve(server: &MockServer, manifest: serde_json::Value, body: &[u8]) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(200).set_body_json(manifest))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(body))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(server)
        .await;
}

fn unplaced(doc: &DocumentId, rel: &str, sibling: &str) -> ConflictNotice {
    ConflictNotice {
        document_id: Some(doc.clone()),
        unplaced: true,
        path: rel.to_owned(),
        sibling: sibling.to_owned(),
        head_revision_id: DocumentRevisionId::generate(),
        detected_at: chrono::Utc::now(),
    }
}

fn with_conflict(fx: &Fixture, notice: ConflictNotice) -> DriveState {
    fx.synced_empty();
    let mut state = fx.state();
    state.conflicts.push(notice);
    state.save(&fx.root.state_path()).expect("save");
    state
}

fn renamed(doc: &DocumentId, from: &str, to: &str) -> serde_json::Value {
    serde_json::json!([{
        "cursor": "c1", "document_id": doc.as_str(), "path": to,
        "kind": {"type": "renamed", "from": from},
    }])
}

#[tokio::test]
async fn a_revisionless_rename_of_an_unplaced_document_places_its_head() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let body: &[u8] = b"was CON.txt";
    let mut state = with_conflict(&fx, unplaced(&doc, "CON.txt", "CON.txt"));
    feed(&server, renamed(&doc, "CON.txt", "notes.txt")).await;
    let rev = DocumentRevisionId::generate();
    serve(
        &server,
        manifest_json(&[(&doc, "notes.txt", &rev, body)]),
        body,
    )
    .await;

    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("notes.txt"), body);
    let saved = fx.state();
    assert!(saved.conflicts.is_empty(), "placed, so cleared");
    assert_eq!(saved.entries["notes.txt"].revision_id, rev);
}

#[tokio::test]
async fn a_revisionless_rename_of_an_unknown_document_places_its_head() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    let doc = DocumentId::generate();
    let body: &[u8] = b"never seen here";
    feed(&server, renamed(&doc, "old.md", "new.md")).await;
    serve(
        &server,
        manifest_json(&[(&doc, "new.md", &DocumentRevisionId::generate(), body)]),
        body,
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("new.md"), body);
}

#[tokio::test]
async fn a_rename_to_another_unplaceable_name_keeps_the_conflict_at_the_new_path() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let body: &[u8] = b"still a device name";
    let mut state = with_conflict(&fx, unplaced(&doc, "CON.txt", "CON.txt"));
    feed(&server, renamed(&doc, "CON.txt", "aux.md")).await;
    serve(
        &server,
        manifest_json(&[(&doc, "aux.md", &DocumentRevisionId::generate(), body)]),
        body,
    )
    .await;

    pull(fx.cycle(), &mut state).await.expect("pull");

    let saved = fx.state();
    let paths: Vec<&str> = saved.conflicts.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, vec!["aux.md"]);
    assert!(saved.conflicts[0].unplaced);
}

#[tokio::test]
async fn an_unreachable_manifest_keeps_the_rename_for_the_next_cycle() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let mut state = with_conflict(&fx, unplaced(&doc, "CON.txt", "CON.txt"));
    feed(&server, renamed(&doc, "CON.txt", "notes.txt")).await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    pull(fx.cycle(), &mut state).await.expect_err("no manifest");

    let saved = fx.state();
    assert_eq!(
        saved.cursor.as_deref(),
        Some("c0"),
        "the change is repeated"
    );
    assert_eq!(saved.conflicts.len(), 1);
}

#[tokio::test]
async fn a_document_whose_name_is_freed_is_placed_on_the_next_pull() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let body: &[u8] = b"the second plan";
    let mut state = with_conflict(&fx, unplaced(&doc, "plan.md", "Plan.md"));
    feed(&server, serde_json::json!([])).await;
    serve(
        &server,
        manifest_json(&[(&doc, "plan.md", &DocumentRevisionId::generate(), body)]),
        body,
    )
    .await;

    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("plan.md"), body);
    assert!(fx.state().conflicts.is_empty());
}

#[tokio::test]
async fn a_rename_onto_an_untracked_file_is_a_conflict_and_moves_nothing() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("mine.md", b"synced");
    fx.synced("mine.md", &doc, &DocumentRevisionId::generate());
    fx.write("taken.md", b"the user's own file");
    feed(&server, renamed(&doc, "mine.md", "taken.md")).await;
    serve(
        &server,
        manifest_json(&[(&doc, "taken.md", &DocumentRevisionId::generate(), b"synced")]),
        b"synced",
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("mine.md"), b"synced");
    assert_eq!(fx.read("taken.md"), b"the user's own file");
    let saved = fx.state();
    assert!(saved.entries.contains_key("mine.md"));
    assert!(!saved.entries.contains_key("taken.md"));
    assert_eq!(saved.conflicts.len(), 1);
    assert_eq!(saved.conflicts[0].path, "taken.md");
}
