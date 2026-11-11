use systemprompt_bridge::drive::pull::pull;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{Fixture, HOST};

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

fn trash_is_empty(fx: &Fixture) -> bool {
    std::fs::read_dir(fx.root.trash_dir()).map_or(true, |mut entries| entries.next().is_none())
}

#[tokio::test]
async fn a_remote_delete_of_a_locally_edited_file_is_kept_as_a_conflict() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    fx.write("notes.md", b"synced");
    fx.synced("notes.md", &doc, &rev);
    fx.write("notes.md", b"edited after the last sync");
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "notes.md",
            "kind": {"type": "deleted"},
        }]),
    )
    .await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    let sibling = format!("notes (conflict from {HOST}).md");
    assert_eq!(fx.read(&sibling), b"edited after the last sync");
    assert!(!fx.root.dir().join("notes.md").exists());
    assert!(
        trash_is_empty(&fx),
        "an edited file never goes to the trash"
    );
    let saved = fx.state();
    assert!(!saved.entries.contains_key("notes.md"));
    assert_eq!(saved.conflicts.len(), 1);
    assert_eq!(saved.conflicts[0].path, "notes.md");
    assert_eq!(saved.conflicts[0].sibling, sibling);
    assert_eq!(saved.conflicts[0].head_revision_id, rev);
    assert_eq!(report.conflicts.len(), 1);
}

#[tokio::test]
async fn a_remote_delete_naming_a_never_synced_file_leaves_it_alone() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    fx.write("draft.md", b"never uploaded");
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": DocumentId::generate().as_str(), "path": "draft.md",
            "kind": {"type": "deleted"},
        }]),
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("draft.md"), b"never uploaded");
    assert!(trash_is_empty(&fx));
    assert_eq!(fx.state().cursor.as_deref(), Some("c9"));
}

#[tokio::test]
async fn a_remote_delete_acts_on_the_recorded_path_not_the_feed_path() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("mine.md", b"synced copy");
    fx.synced("mine.md", &doc, &DocumentRevisionId::generate());
    fx.write("other.md", b"unrelated local file");
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "other.md",
            "kind": {"type": "deleted"},
        }]),
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("other.md"), b"unrelated local file");
    assert!(!fx.root.dir().join("mine.md").exists());
    assert!(!fx.state().entries.contains_key("mine.md"));
}

#[tokio::test]
async fn a_rename_moves_the_recorded_path_not_the_feed_from() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("mine.md", b"the document");
    fx.synced("mine.md", &doc, &DocumentRevisionId::generate());
    fx.write("bystander.md", b"not this one");
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "moved.md",
            "kind": {"type": "renamed", "from": "bystander.md"},
        }]),
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("bystander.md"), b"not this one");
    assert_eq!(fx.read("moved.md"), b"the document");
    assert!(!fx.root.dir().join("mine.md").exists());
}

#[tokio::test]
async fn a_case_variant_of_a_synced_file_is_a_conflict_not_an_overwrite() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("Plan.md", b"the first plan");
    fx.synced("Plan.md", &doc, &DocumentRevisionId::generate());
    let other_rev = DocumentRevisionId::generate();
    let body: &[u8] = b"a different document";
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": DocumentId::generate().as_str(), "path": "plan.md",
            "revision_id": other_rev.as_str(), "content_sha256": content_hash(body).as_str(),
            "kind": {"type": "created"},
        }]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(body))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("Plan.md"), b"the first plan");
    let names: Vec<String> = std::fs::read_dir(fx.root.dir())
        .expect("list the drive root")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.eq_ignore_ascii_case("plan.md"))
        .collect();
    assert_eq!(names, ["Plan.md"], "no second case variant is written");
    let saved = fx.state();
    assert!(!saved.entries.contains_key("plan.md"));
    assert_eq!(saved.conflicts.len(), 1);
    assert_eq!(saved.conflicts[0].path, "plan.md");
    assert_eq!(saved.conflicts[0].sibling, "Plan.md");
    assert_eq!(saved.conflicts[0].head_revision_id, other_rev);
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(
        saved.cursor.as_deref(),
        Some("c9"),
        "the feed moves past it"
    );
}
