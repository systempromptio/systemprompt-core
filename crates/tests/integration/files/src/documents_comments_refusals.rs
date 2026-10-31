use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{CommentAnchor, CommentRange};
use systemprompt_files::documents::service::{
    CommentRequest, DeleteCommentRequest, EditCommentRequest,
};
use systemprompt_identifiers::{DocumentCommentId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
};

async fn commented(h: &Harness, owner: &AuthenticatedUser) -> (DocumentId, CommentRequest) {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "alpha beta\n".to_owned(),
    }];
    let document = h
        .service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id;
    let (_, projection) = h
        .service
        .project(owner, &document, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line");
    let comment = CommentRequest {
        document: document.clone(),
        anchor: Some(CommentAnchor {
            node: node.id.clone(),
            find: None,
            nth: None,
        }),
        range: None,
        body: "plain".to_owned(),
        mentions: Vec::new(),
        parent_id: None,
        mention_options: vec![],
    };
    (document, comment)
}

fn edit(document: &DocumentId, comment: &DocumentCommentId, body: String) -> EditCommentRequest {
    EditCommentRequest {
        document: document.clone(),
        comment: comment.clone(),
        body,
        mentions: Vec::new(),
        mention_options: vec![],
    }
}

fn invalid(error: &DocumentsError) -> bool {
    matches!(error, DocumentsError::InvalidComment { .. })
}

#[tokio::test]
async fn malformed_tokens_unbounded_ranges_and_crowded_mentions_are_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("refuse-shape");
    let (_, base) = commented(&h, &owner).await;
    let mut cases = Vec::new();
    for body in ["see @[](user:someone)", "see @[a(b](user:someone)"] {
        let mut request = base.clone();
        request.body = body.to_owned();
        cases.push(request);
    }
    let mut empty_range = base.clone();
    empty_range.range = Some(CommentRange {
        start: String::new(),
        end: "end".to_owned(),
    });
    cases.push(empty_range);
    let mut long_range = base.clone();
    long_range.range = Some(CommentRange {
        start: "start".to_owned(),
        end: "x".repeat(1025),
    });
    cases.push(long_range);
    let mut crowded = base.clone();
    crowded.body = (0..21).fold(String::from("all"), |body, n| {
        format!("{body} @[U{n}](user:crowd-{n})")
    });
    cases.push(crowded);
    for request in cases {
        let error = h
            .service
            .create_comment(&owner, request, &trace())
            .await
            .expect_err("refused");
        assert!(invalid(&error), "{error:?}");
    }
    let listed = h
        .service
        .comments(&owner, &base.document, &trace())
        .await
        .expect("list");
    assert!(listed.is_empty(), "nothing refused was stored");
}

#[tokio::test]
async fn an_edit_that_mentions_a_stranger_shares_with_them_and_only_the_author_edits() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("refuse-edit-owner");
    let stranger = person("refuse-edit-stranger");
    let (document, request) = commented(&h, &owner).await;
    let comment = h
        .service
        .create_comment(&owner, request, &trace())
        .await
        .expect("comment");
    let mut mention = edit(
        &document,
        &comment.id,
        format!("ping @[Them](user:{})", stranger.id),
    );
    mention.mentions = vec![stranger.id.clone()];
    let edited = h
        .service
        .edit_comment(&owner, mention, &trace())
        .await
        .expect("edit");
    assert_eq!(edited.mentions, vec![stranger.id.clone()]);
    let levels = h
        .service
        .levels_on(
            &owner,
            &document,
            std::slice::from_ref(&stranger.id),
            &trace(),
        )
        .await
        .expect("levels");
    assert_eq!(levels, vec![Some(AccessLevel::Comment)]);

    let error = h
        .service
        .edit_comment(
            &stranger,
            edit(&document, &comment.id, "hijack".to_owned()),
            &trace(),
        )
        .await
        .expect_err("not the author");
    assert!(invalid(&error), "{error:?}");
    h.service
        .delete_comment(
            &stranger,
            DeleteCommentRequest {
                document: document.clone(),
                comment: comment.id.clone(),
            },
            &trace(),
        )
        .await
        .expect_err("deleting another's comment needs share");
    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    let kept = listed
        .iter()
        .find(|listed| listed.id == comment.id)
        .expect("kept");
    assert_eq!(kept.body, format!("ping @[Them](user:{})", stranger.id));
}

#[tokio::test]
async fn a_deleted_comment_can_be_neither_edited_nor_deleted_again() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("refuse-deleted");
    let (document, request) = commented(&h, &owner).await;
    let comment = h
        .service
        .create_comment(&owner, request, &trace())
        .await
        .expect("comment");
    let delete = DeleteCommentRequest {
        document: document.clone(),
        comment: comment.id.clone(),
    };
    h.service
        .delete_comment(&owner, delete.clone(), &trace())
        .await
        .expect("delete");
    let again = h
        .service
        .delete_comment(&owner, delete, &trace())
        .await
        .expect_err("gone");
    assert!(matches!(again, DocumentsError::CommentNotFound { .. }));
    let edited = h
        .service
        .edit_comment(
            &owner,
            edit(&document, &comment.id, "late".to_owned()),
            &trace(),
        )
        .await
        .expect_err("gone");
    assert!(matches!(edited, DocumentsError::CommentNotFound { .. }));
    let unknown = h
        .service
        .edit_comment(
            &owner,
            edit(
                &document,
                &DocumentCommentId::new("00000000-0000-4000-8000-00000000c0de"),
                "nobody".to_owned(),
            ),
            &trace(),
        )
        .await
        .expect_err("unknown");
    assert!(!invalid(&unknown), "{unknown:?}");
}
