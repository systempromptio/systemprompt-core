use systemprompt_bridge::drive::pull::pull;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use crate::support::Fixture;

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

struct EditWhileDownloading {
    file: std::path::PathBuf,
    body: Vec<u8>,
}

impl Respond for EditWhileDownloading {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        std::fs::write(&self.file, b"typed during the download").expect("local edit");
        ResponseTemplate::new(200).set_body_bytes(self.body.clone())
    }
}

#[tokio::test]
async fn renamed_document_moves_the_file_and_its_entry() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("drafts/plan.md", b"the plan");
    fx.synced("drafts/plan.md", &doc, &DocumentRevisionId::generate());
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "final/plan v2.md",
            "kind": {"type": "renamed", "from": "drafts/plan.md"},
        }]),
    )
    .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert!(!fx.root.dir().join("drafts/plan.md").exists());
    assert_eq!(fx.read("final/plan v2.md"), b"the plan");
    let saved = fx.state();
    assert!(!saved.entries.contains_key("drafts/plan.md"));
    assert_eq!(
        saved.entries["final/plan v2.md"].document_id.as_ref(),
        Some(&doc)
    );
}

#[tokio::test]
async fn an_update_at_a_new_path_moves_the_known_file_first() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("a.md", b"old");
    fx.synced("a.md", &doc, &DocumentRevisionId::generate());
    let body: &[u8] = b"new";
    feed(
        &server,
        serde_json::json!([{
            "cursor": "c1", "document_id": doc.as_str(), "path": "moved/a.md",
            "revision_id": DocumentRevisionId::generate().as_str(),
            "content_sha256": content_hash(body).as_str(), "kind": {"type": "updated"},
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

    assert!(!fx.root.dir().join("a.md").exists());
    assert_eq!(fx.read("moved/a.md"), body);
    assert_eq!(fx.state().entries.len(), 1);
}

#[tokio::test]
async fn a_local_edit_made_during_the_download_is_kept_beside() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let rev = DocumentRevisionId::generate();
    fx.write("a.md", b"synced");
    fx.synced("a.md", &doc, &DocumentRevisionId::generate());
    let body: &[u8] = b"remote update";
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
        .respond_with(EditWhileDownloading {
            file: fx.root.dir().join("a.md"),
            body: body.to_vec(),
        })
        .mount(&server)
        .await;

    let mut state = fx.state();
    let report = pull(fx.cycle(), &mut state).await.expect("pull");

    assert_eq!(fx.read("a.md"), body);
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(
        fx.read(&report.conflicts[0].sibling),
        b"typed during the download"
    );
    assert_eq!(fx.state().entries["a.md"].revision_id, rev);
}
