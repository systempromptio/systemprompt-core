use std::sync::Arc;
use std::time::Duration;

use loro::{ExportMode, LoroDoc, VersionVector};
use systemprompt_events::Broadcaster;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::collab::model::BODY_CONTAINER;
use systemprompt_files::documents::collab::{
    ApplyRequest, CollabError, CollabService, CollabTuning, JoinMode, Joined,
};
use systemprompt_identifiers::{ConnectionId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::RevisionOriginKind;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
};

struct Collab {
    h: Harness,
    documents: Arc<systemprompt_files::documents::DocumentService>,
    collab: CollabService,
}

async fn collab() -> Collab {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let documents = Arc::new(h.service.clone());
    let collab = CollabService::new(Arc::clone(&documents), CollabTuning::default());
    Collab {
        h,
        documents,
        collab,
    }
}

async fn html_document(c: &Collab, owner: &AuthenticatedUser) -> DocumentId {
    let (document, _) = c
        .documents
        .create_from_template(owner, create("blank-html", None), &trace())
        .await
        .expect("create html");
    document.id
}

fn client(joined: &Joined) -> LoroDoc {
    let doc = LoroDoc::new();
    doc.import(&joined.snapshot).expect("import snapshot");
    doc
}

fn edit(doc: &LoroDoc, text: &str) -> Vec<u8> {
    let before = doc.oplog_vv();
    let body = doc.get_text(BODY_CONTAINER);
    body.insert(body.len_unicode(), text).expect("insert");
    doc.commit();
    doc.export(ExportMode::updates(&before))
        .expect("export update")
}

fn request(document: &DocumentId, joined: &Joined, doc: &LoroDoc, update: Vec<u8>) -> ApplyRequest {
    ApplyRequest {
        document_id: document.clone(),
        session_id: joined.session_id.clone(),
        version: Some(doc.oplog_vv().encode()),
        update,
    }
}

#[tokio::test]
async fn join_opens_one_session_and_a_second_join_reuses_it() {
    let c = collab().await;
    let owner = person("collab-owner");
    let document = html_document(&c, &owner).await;
    let first = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("first join");
    let second = c
        .collab
        .join(&owner, &document, JoinMode::View, &trace())
        .await
        .expect("second join");
    assert_eq!(first.session_id, second.session_id);
    let open =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find open")
            .expect("open session");
    assert_eq!(open.id, first.session_id);
    assert!(first.head_revision.is_some());
    assert!(!first.snapshot.is_empty());
    VersionVector::decode(&first.version).expect("version vector");
}

#[tokio::test]
async fn apply_logs_the_update_and_broadcasts_it_to_present_users() {
    let c = collab().await;
    let owner = person("collab-apply");
    let document = html_document(&c, &owner).await;
    let (tx, mut rx) = mpsc::channel(16);
    assert!(
        c.h.broadcasters
            .agui
            .register(&owner.id, &ConnectionId::generate(), tx)
            .await
    );
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Live</p>");
    let applied = c
        .collab
        .apply(
            &owner,
            request(&document, &joined, &doc, update.clone()),
            &trace(),
        )
        .await
        .expect("apply");
    assert!(applied.seq > 0);
    assert!(applied.missing.is_none());
    let rows =
        c.h.repos
            .collab
            .list_after(&joined.session_id, 0)
            .await
            .expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].seq, applied.seq);
    assert_eq!(rows[0].update, update);
    assert_eq!(rows[0].author_user_id, owner.id);

    let mut saw_update = false;
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        if format!("{event:?}").contains("document.collab.update") {
            saw_update = true;
        }
    }
    assert!(
        saw_update,
        "the owner's stream must carry document.collab.update"
    );
}

#[tokio::test]
async fn a_second_replica_sees_updates_applied_on_the_first() {
    let c = collab().await;
    let other = CollabService::new(Arc::clone(&c.documents), CollabTuning::default());
    let owner = person("collab-replica");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Shared</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    let rejoined = other
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join on second replica");
    let mirror = client(&rejoined);
    assert_eq!(
        mirror.get_text(BODY_CONTAINER).to_string(),
        doc.get_text(BODY_CONTAINER).to_string()
    );
}

#[tokio::test]
async fn two_hundred_updates_store_a_snapshot_and_prune_the_log() {
    let c = collab().await;
    let owner = person("collab-snapshot");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    for i in 0..200 {
        let update = edit(&doc, &format!("{}", i % 10));
        c.collab
            .apply(&owner, request(&document, &joined, &doc, update), &trace())
            .await
            .expect("apply");
    }
    let open =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("open");
    assert!(open.snapshot_sha256.is_some());
    let left =
        c.h.repos
            .collab
            .list_after(&joined.session_id, 0)
            .await
            .expect("rows");
    assert!(left.is_empty(), "the snapshot covers every logged update");
}

#[tokio::test]
async fn the_last_editor_leaving_commits_an_edit_session_revision_and_closes() {
    let c = collab().await;
    let owner = person("collab-leave");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Checkpointed</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    let left = c
        .collab
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("leave");
    assert!(left.closed);
    let revision_id = left.checkpoint.expect("checkpoint revision");
    let revision =
        c.h.repos
            .revisions
            .find(&revision_id)
            .await
            .expect("find")
            .expect("revision");
    assert_eq!(revision.origin_kind, RevisionOriginKind::EditSession);
    assert_eq!(
        revision.origin_id.as_deref(),
        Some(joined.session_id.as_str())
    );
    let (_, bytes) = c
        .documents
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("read");
    let html = String::from_utf8(bytes).expect("utf8");
    assert!(html.contains("<p>Checkpointed</p></body>"));
    assert!(html.contains("<head>"));
    assert!(
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .is_none()
    );
}

#[tokio::test]
async fn office_documents_cannot_be_joined() {
    let c = collab().await;
    let owner = person("collab-office");
    let (document, _) = c
        .documents
        .create_from_template(&owner, create("blank-docx", None), &trace())
        .await
        .expect("create docx");
    let refused = c
        .collab
        .join(&owner, &document.id, JoinMode::Edit, &trace())
        .await;
    assert!(matches!(
        refused,
        Err(CollabError::Documents(
            DocumentsError::UnsupportedFormat { .. }
        ))
    ));
}

#[tokio::test]
async fn a_view_mode_join_cannot_apply_updates() {
    let c = collab().await;
    let owner = person("collab-view");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::View, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>no</p>");
    let refused = c
        .collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await;
    assert!(matches!(refused, Err(CollabError::NotEditing { .. })));
}
