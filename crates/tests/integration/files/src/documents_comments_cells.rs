use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::xlsx::checkpoint_xlsx;
use systemprompt_files::documents::engine::loro::sheet::{Sheet, SheetCell, SheetModel};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::{CellRange, CommentAnchor, CommentRange};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommentRequest, CommitRequest, ImportRequest,
};
use systemprompt_identifiers::{DocumentCommentId, DocumentId, DocumentNodeId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    CellValue, DocumentFormat, EditOp, NodeKind, Projection, ProjectionNode,
};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, harness, person, trace, unique,
};

fn cells(text: &str) -> CellRange {
    text.parse().expect("cell range")
}

fn workbook(names: &[&str]) -> Vec<u8> {
    let sheets = names
        .iter()
        .map(|name| {
            let cell = SheetCell {
                v: format!("{name} title"),
                ..SheetCell::default()
            };
            Sheet::from_grid(name, [(1, 1, cell)], &[])
        })
        .collect();
    checkpoint_xlsx(&SheetModel { sheets }).expect("workbook bytes")
}

async fn imported(h: &Harness, owner: &AuthenticatedUser, names: &[&str]) -> DocumentId {
    h.service
        .import(
            owner,
            ImportRequest {
                folder: None,
                name: unique("cells"),
                format: Some(DocumentFormat::Xlsx),
                mime_type: None,
                bytes: workbook(names),
                author: api_author(),
                message: None,
            },
            &trace(),
        )
        .await
        .expect("import workbook")
        .0
        .id
}

async fn projection(h: &Harness, owner: &AuthenticatedUser, document: &DocumentId) -> Projection {
    h.service
        .project(owner, document, None, &trace())
        .await
        .expect("project")
        .1
}

fn sheet<'p>(projection: &'p Projection, name: &str) -> &'p ProjectionNode {
    projection
        .nodes()
        .find(|node| node.kind == NodeKind::Sheet && node.text.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no sheet {name}"))
}

fn anchored(
    document: &DocumentId,
    node: &DocumentNodeId,
    find: Option<&str>,
    range: Option<CommentRange>,
) -> CommentRequest {
    CommentRequest {
        document: document.clone(),
        anchor: Some(CommentAnchor {
            node: node.clone(),
            find: find.map(str::to_owned),
            nth: None,
            cells: Some(cells("B2:D9")),
        }),
        range,
        body: "check these figures".to_owned(),
        mentions: Vec::new(),
        mention_options: vec![],
        parent_id: None,
    }
}

fn commit(document: &DocumentId, body: CommitBody, comments: CommentPolicy) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body,
        author: api_author(),
        message: None,
        lock_holder: None,
        comments,
        op_id: None,
        media: Media::new(),
    }
}

async fn listed(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    comment: &DocumentCommentId,
) -> CommentAnchor {
    h.service
        .comments(owner, document, &trace())
        .await
        .expect("list")
        .into_iter()
        .find(|listed| listed.id == *comment)
        .expect("comment listed")
        .anchor
}

#[tokio::test]
async fn a_cell_range_comment_survives_cell_edits_and_detaches_with_its_sheet() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("cells-sheet");
    let document = imported(&h, &owner, &["Data", "Other"]).await;
    let head = projection(&h, &owner, &document).await;
    let data = sheet(&head, "Data").id.clone();
    let comment = h
        .service
        .create_comment(&owner, anchored(&document, &data, None, None), &trace())
        .await
        .expect("cell range comment");
    assert_eq!(comment.anchor.cells, Some(cells("B2:D9")));
    assert_eq!(comment.anchor.node, data);

    let set = EditOp::SetCells {
        sheet: "Data".to_owned(),
        range: "A1:C3".to_owned(),
        values: vec![vec![CellValue::Text("rewritten".to_owned()); 3]; 3],
        expected_hash: None,
    };
    h.service
        .commit(
            &owner,
            commit(
                &document,
                CommitBody::Ops(vec![set]),
                CommentPolicy::default(),
            ),
            &trace(),
        )
        .await
        .expect("a cell edit leaves the range in place");
    assert_eq!(
        listed(&h, &owner, &document, &comment.id).await.cells,
        Some(cells("B2:D9"))
    );

    let base = h
        .service
        .project(&owner, &document, None, &trace())
        .await
        .expect("head")
        .0
        .id;
    let without_data = |comments| CommitRequest {
        base_revision: Some(base.clone()),
        ..commit(&document, CommitBody::Bytes(workbook(&["Other"])), comments)
    };
    let refused = h
        .service
        .commit(&owner, without_data(CommentPolicy::default()), &trace())
        .await
        .expect_err("deleting the sheet affects the anchor");
    let DocumentsError::AnchorsAffected { comments } = refused else {
        panic!("expected anchors_affected, got {refused:?}");
    };
    assert_eq!(comments, vec![comment.id.clone()]);

    h.service
        .commit(&owner, without_data(CommentPolicy::detach()), &trace())
        .await
        .expect("detach allowed");
    let detached = listed(&h, &owner, &document, &comment.id).await;
    assert_eq!(detached.node, data, "a detached comment keeps its node");
    assert!(
        detached.cells.is_none(),
        "a detached comment loses its cells"
    );
}

#[tokio::test]
async fn a_reply_carries_the_cell_range_of_its_parent() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("cells-reply");
    let document = imported(&h, &owner, &["Data"]).await;
    let head = projection(&h, &owner, &document).await;
    let data = sheet(&head, "Data").id.clone();
    let root = h
        .service
        .create_comment(&owner, anchored(&document, &data, None, None), &trace())
        .await
        .expect("root");
    let reply = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                anchor: None,
                parent_id: Some(root.id.clone()),
                ..anchored(&document, &data, None, None)
            },
            &trace(),
        )
        .await
        .expect("reply");
    assert_eq!(reply.anchor.cells, Some(cells("B2:D9")));
}

#[tokio::test]
async fn a_cell_range_is_refused_off_a_sheet_and_beside_a_quote_or_range() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("cells-refused");
    let markdown = h
        .service
        .import(
            &owner,
            ImportRequest {
                folder: None,
                name: unique("notes"),
                format: Some(DocumentFormat::Markdown),
                mime_type: None,
                bytes: b"alpha beta\n".to_vec(),
                author: api_author(),
                message: None,
            },
            &trace(),
        )
        .await
        .expect("markdown")
        .0
        .id;
    let head = projection(&h, &owner, &markdown).await;
    let line = head
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line")
        .id
        .clone();
    let off_sheet = h
        .service
        .create_comment(&owner, anchored(&markdown, &line, None, None), &trace())
        .await
        .expect_err("markdown has no sheet");
    assert!(
        matches!(off_sheet, DocumentsError::InvalidComment { .. }),
        "{off_sheet:?}"
    );

    let spreadsheet = imported(&h, &owner, &["Data"]).await;
    let head = projection(&h, &owner, &spreadsheet).await;
    let data = sheet(&head, "Data").id.clone();
    let with_quote = h
        .service
        .create_comment(
            &owner,
            anchored(&spreadsheet, &data, Some("Data"), None),
            &trace(),
        )
        .await
        .expect_err("cells and find together");
    assert!(
        matches!(with_quote, DocumentsError::InvalidComment { .. }),
        "{with_quote:?}"
    );
    let cursors = CommentRange {
        start: "cursor-start".to_owned(),
        end: "cursor-end".to_owned(),
    };
    let with_range = h
        .service
        .create_comment(
            &owner,
            anchored(&spreadsheet, &data, None, Some(cursors)),
            &trace(),
        )
        .await
        .expect_err("cells and a cursor range together");
    assert!(
        matches!(with_range, DocumentsError::InvalidComment { .. }),
        "{with_range:?}"
    );
    let cell = head
        .nodes()
        .find(|node| node.kind == NodeKind::Cell)
        .expect("a cell node")
        .id
        .clone();
    let on_cell = h
        .service
        .create_comment(&owner, anchored(&spreadsheet, &cell, None, None), &trace())
        .await
        .expect_err("a range anchors to the sheet, not a cell");
    assert!(
        matches!(on_cell, DocumentsError::InvalidComment { .. }),
        "{on_cell:?}"
    );
    assert!(
        h.service
            .comments(&owner, &spreadsheet, &trace())
            .await
            .expect("list")
            .is_empty()
    );
}
