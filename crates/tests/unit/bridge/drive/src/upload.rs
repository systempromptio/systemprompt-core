use systemprompt_bridge::drive::upload::upload;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{Fixture, manifest_json};

#[tokio::test]
async fn upload_sends_if_match_and_records_the_committed_revision() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let base = DocumentRevisionId::generate();
    let next = DocumentRevisionId::generate();
    fx.write("plan.md", b"v1");
    fx.synced("plan.md", &doc, &base);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fx.write("plan.md", b"v2 edited locally");

    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_COMMIT))
        .and(header(
            "if-match",
            format!("\"{}\"", base.as_str()).as_str(),
        ))
        .and(header("authorization", "Bearer drive-bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "committed", "revision_id": next.as_str(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut state = fx.state();
    let report = upload(fx.cycle(), &mut state).await.expect("upload");
    assert_eq!(report.committed, 1);

    let saved = fx.state();
    let entry = saved.entries.get("plan.md").expect("entry");
    assert_eq!(entry.revision_id, next);
    assert_eq!(entry.sha256, content_hash(b"v2 edited locally"));
    assert_eq!(entry.document_id.as_ref(), Some(&doc));

    let requests = server.received_requests().await.expect("requests");
    let body = String::from_utf8_lossy(&requests[0].body).into_owned();
    assert!(
        body.contains("name=\"meta\""),
        "multipart meta part: {body}"
    );
    assert!(body.contains("name=\"blob\""), "multipart blob part");
    assert!(body.contains(base.as_str()), "meta names the base revision");
    assert!(body.contains("v2 edited locally"), "blob carries the bytes");
}

#[tokio::test]
async fn conflict_keeps_local_edit_as_sibling_and_writes_the_verified_head() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let base = DocumentRevisionId::generate();
    let head = DocumentRevisionId::generate();
    fx.write("docs/brief.docx", b"base");
    fx.synced("docs/brief.docx", &doc, &base);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fx.write("docs/brief.docx", b"mine");
    let theirs: &[u8] = b"theirs, committed elsewhere";

    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_COMMIT))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "status": "conflict",
            "head_revision_id": head.as_str(),
            "head_content_sha256": content_hash(theirs).as_str(),
            "base_revision_id": base.as_str(),
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(200).set_body_json(manifest_json(&[(
            &doc,
            "docs/brief.docx",
            &head,
            theirs,
        )])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(theirs))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(theirs.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    let report = upload(fx.cycle(), &mut state).await.expect("upload");

    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(
        report.conflicts[0].sibling,
        "docs/brief (conflict from laptop).docx"
    );
    assert_eq!(fx.read("docs/brief (conflict from laptop).docx"), b"mine");
    assert_eq!(fx.read("docs/brief.docx"), theirs);
    let saved = fx.state();
    assert_eq!(saved.entries["docs/brief.docx"].revision_id, head);
    assert_eq!(saved.conflicts.len(), 1);
}

#[tokio::test]
async fn a_merged_commit_is_recorded_and_the_next_pull_brings_the_merge_down() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    let base = DocumentRevisionId::generate();
    let merged = DocumentRevisionId::generate();
    fx.write("docs/brief.docx", b"base");
    fx.synced("docs/brief.docx", &doc, &base);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    fx.write("docs/brief.docx", b"mine");
    let combined: &[u8] = b"mine and theirs, merged on the server";

    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_COMMIT))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "merged",
            "revision_id": merged.as_str(),
            "content_sha256": content_hash(combined).as_str(),
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(combined))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(combined.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    let report = upload(fx.cycle(), &mut state).await.expect("upload");
    assert_eq!(report.merged, 1);
    assert!(report.conflicts.is_empty());
    let entry = &fx.state().entries["docs/brief.docx"];
    assert_eq!(entry.revision_id, merged);
    assert_eq!(entry.sha256, content_hash(b"mine"));

    let manifest: systemprompt_models::documents::DriveManifest = serde_json::from_value(
        manifest_json(&[(&doc, "docs/brief.docx", &merged, combined)]),
    )
    .expect("manifest");
    let mut state = fx.state();
    let pulled = systemprompt_bridge::drive::pull::reconcile(fx.cycle(), &mut state, manifest)
        .await
        .expect("pull");
    assert!(pulled.conflicts.is_empty());
    assert_eq!(fx.read("docs/brief.docx"), combined);
}
