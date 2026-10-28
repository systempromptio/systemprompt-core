use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::CommentAnchor;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommentRequest, CommitRequest, HistoryPage, ResolveCommentRequest,
};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentNodeId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{CellValue, EditOp};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};

async fn document(
    h: &Harness,
    owner: &AuthenticatedUser,
    template: &str,
    text: &str,
) -> DocumentId {
    let mut request = create(template, None);
    request.name = unique("reads");
    request.ops = vec![EditOp::SetText {
        text: text.to_owned(),
    }];
    if template != "blank-text" {
        request.ops = vec![EditOp::AppendMarkdown {
            markdown: text.to_owned(),
        }];
    }
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

fn commit(document: &DocumentId, ops: Vec<EditOp>) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(ops),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    }
}

#[tokio::test]
async fn revision_reads_its_own_document_only() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("reads-revision");
    let first = document(&h, &owner, "blank-text", "one\n").await;
    let second = document(&h, &owner, "blank-text", "two\n").await;
    let history = h
        .service
        .history(
            &owner,
            &first,
            HistoryPage {
                limit: 10,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    let head = history.first().expect("revision").clone();
    let read = h
        .service
        .revision(&owner, &first, &head.id, &trace())
        .await
        .expect("revision");
    assert_eq!(read.id, head.id);
    assert_eq!(read.document_id, first);
    let foreign = h
        .service
        .revision(&owner, &second, &head.id, &trace())
        .await
        .expect_err("revision of another document");
    assert!(
        matches!(
            foreign,
            DocumentsError::NotFound {
                what: "revision",
                ..
            }
        ),
        "{foreign:?}"
    );
}

#[tokio::test]
async fn search_finds_indexed_text_and_drops_trashed_documents() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("reads-search");
    let word = unique("zebra").replace('-', "");
    let kept = document(&h, &owner, "blank-text", &format!("the {word} line\n")).await;
    let trashed = document(&h, &owner, "blank-text", &format!("another {word}\n")).await;
    let found = h
        .service
        .search(&owner, &word, 20, &trace())
        .await
        .expect("search");
    let ids: Vec<_> = found.iter().map(|r| r.document.id.clone()).collect();
    assert!(ids.contains(&kept), "{ids:?}");
    assert!(ids.contains(&trashed));
    assert!(found.iter().all(|r| r.hit.document_id == r.document.id));

    h.service
        .trash(&owner, &trashed, &trace())
        .await
        .expect("trash");
    let after = h
        .service
        .search(&owner, &word, 20, &trace())
        .await
        .expect("search");
    let ids: Vec<_> = after.iter().map(|r| r.document.id.clone()).collect();
    assert!(ids.contains(&kept));
    assert!(!ids.contains(&trashed));
    let stranger = person("reads-stranger");
    let none = h
        .service
        .search(&stranger, &word, 20, &trace())
        .await
        .expect("search");
    assert!(none.is_empty());
}

#[tokio::test]
async fn comments_anchor_on_nested_text_and_refuse_bad_requests() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("reads-comments");
    let doc = document(
        &h,
        &owner,
        "blank-markdown",
        "- apple pie\n- banana split\n",
    )
    .await;
    let (_, projection) = h
        .service
        .project(&owner, &doc, None, &trace())
        .await
        .expect("project");
    let parent = projection
        .nodes()
        .find(|n| !n.children.is_empty())
        .expect("node with children")
        .clone();
    let anchored = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                range: None,
                mentions: Vec::new(),
                document: doc.clone(),
                anchor: Some(CommentAnchor {
                    node: parent.id.clone(),
                    find: Some("banana".to_owned()),
                    nth: None,
                }),
                body: "nested".to_owned(),
                parent_id: None,
            },
            &trace(),
        )
        .await
        .expect("comment on nested text");
    assert_eq!(anchored.anchor.node, parent.id);

    let request =
        |anchor: Option<CommentAnchor>, parent_id: Option<DocumentCommentId>| CommentRequest {
            range: None,
            mentions: Vec::new(),
            document: doc.clone(),
            anchor,
            body: "x".to_owned(),
            parent_id,
        };
    let neither = h
        .service
        .create_comment(&owner, request(None, None), &trace())
        .await
        .expect_err("no anchor");
    assert!(matches!(neither, DocumentsError::InvalidComment { .. }));
    let unknown = h
        .service
        .create_comment(
            &owner,
            request(
                Some(CommentAnchor {
                    node: DocumentNodeId::new("nope"),
                    find: None,
                    nth: None,
                }),
                None,
            ),
            &trace(),
        )
        .await
        .expect_err("unknown node");
    assert!(matches!(unknown, DocumentsError::UnknownNode { .. }));
    let missing_find = h
        .service
        .create_comment(
            &owner,
            request(
                Some(CommentAnchor {
                    node: parent.id.clone(),
                    find: Some("banana".to_owned()),
                    nth: Some(3),
                }),
                None,
            ),
            &trace(),
        )
        .await
        .expect_err("nth beyond matches");
    assert!(matches!(missing_find, DocumentsError::FindNone { .. }));
    let orphan = h
        .service
        .create_comment(
            &owner,
            request(None, Some(DocumentCommentId::generate())),
            &trace(),
        )
        .await
        .expect_err("unknown parent");
    assert!(matches!(orphan, DocumentsError::CommentNotFound { .. }));
    let resolve_missing = h
        .service
        .resolve_comment(
            &owner,
            ResolveCommentRequest {
                document: doc.clone(),
                comment: DocumentCommentId::generate(),
                answering: None,
            },
            &trace(),
        )
        .await
        .expect_err("unknown comment");
    assert!(matches!(
        resolve_missing,
        DocumentsError::CommentNotFound { .. }
    ));
}

#[tokio::test]
async fn codec_refusals_surface_as_document_errors() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("reads-apply");
    let doc = document(&h, &owner, "blank-text", "only line\n").await;
    let unsupported = h
        .service
        .commit(
            &owner,
            commit(
                &doc,
                vec![EditOp::SetCells {
                    sheet: "Sheet1".to_owned(),
                    range: "A1".to_owned(),
                    values: vec![vec![CellValue::Number(1.0)]],
                    expected_hash: None,
                }],
            ),
            &trace(),
        )
        .await
        .expect_err("set_cells on text");
    assert!(
        matches!(
            unsupported,
            DocumentsError::UnsupportedOp {
                op: "set_cells",
                ..
            }
        ),
        "{unsupported:?}"
    );
    let unknown = h
        .service
        .commit(
            &owner,
            commit(
                &doc,
                vec![EditOp::InsertSlide {
                    after: Some(DocumentNodeId::new("ghost")),
                    layout: None,
                    title: None,
                    body: Vec::new(),
                }],
            ),
            &trace(),
        )
        .await
        .expect_err("unknown anchor");
    assert!(
        matches!(unknown, DocumentsError::UnknownNode { .. }),
        "{unknown:?}"
    );
}
