use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{CommentAnchor, CommentRange};
use systemprompt_files::documents::service::{
    CommentRequest, DeleteCommentRequest, EditCommentRequest,
};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp, Projection};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace,
};

async fn text_document(
    h: &Harness,
    owner: &AuthenticatedUser,
    text: &str,
) -> (systemprompt_identifiers::DocumentId, Projection) {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: text.to_owned(),
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
    (document, projection)
}

fn tokens(text: &str, mentions: &[systemprompt_identifiers::UserId]) -> String {
    let mut body = text.to_owned();
    for user in mentions {
        body.push_str(&format!(" @[Someone](user:{user})"));
    }
    body
}

fn request(
    document: &systemprompt_identifiers::DocumentId,
    projection: &Projection,
    mentions: Vec<systemprompt_identifiers::UserId>,
) -> CommentRequest {
    let node = projection
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line");
    CommentRequest {
        document: document.clone(),
        anchor: Some(CommentAnchor {
            node: node.id.clone(),
            find: None,
            nth: None,
        }),
        range: Some(CommentRange {
            start: "cursor-start".to_owned(),
            end: "cursor-end".to_owned(),
        }),
        body: tokens("look here", &mentions),
        mentions,
        parent_id: None,
    }
}

#[tokio::test]
async fn a_comment_keeps_its_range_and_mentions_and_is_edited_then_deleted() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-v2-owner");
    let (document, projection) = text_document(&h, &owner, "alpha beta\n").await;
    let comment = h
        .service
        .create_comment(
            &owner,
            request(&document, &projection, vec![owner.id.clone()]),
            &trace(),
        )
        .await
        .expect("comment");
    assert_eq!(
        comment.range,
        Some(CommentRange {
            start: "cursor-start".to_owned(),
            end: "cursor-end".to_owned(),
        })
    );
    assert_eq!(comment.mentions, vec![owner.id.clone()]);

    let edited = h
        .service
        .edit_comment(
            &owner,
            EditCommentRequest {
                document: document.clone(),
                comment: comment.id.clone(),
                body: "look again".to_owned(),
                mentions: Vec::new(),
            },
            &trace(),
        )
        .await
        .expect("edit");
    assert_eq!(edited.body, "look again");
    assert!(edited.edited_at.is_some());
    assert!(edited.mentions.is_empty());

    h.service
        .delete_comment(
            &owner,
            DeleteCommentRequest {
                document: document.clone(),
                comment: comment.id.clone(),
            },
            &trace(),
        )
        .await
        .expect("delete");
    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    assert!(listed.iter().all(|listed| listed.id != comment.id));
}

#[tokio::test]
async fn mentioning_a_user_without_access_shares_the_document_as_comment() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-v2-mentioner");
    let stranger = person("comment-v2-stranger");
    let (document, projection) = text_document(&h, &owner, "alpha beta\n").await;
    let comment = h
        .service
        .create_comment(
            &owner,
            request(&document, &projection, vec![stranger.id.clone()]),
            &trace(),
        )
        .await
        .expect("comment");
    assert_eq!(comment.mentions, vec![stranger.id.clone()]);
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
}

#[tokio::test]
async fn a_mention_id_missing_from_the_body_is_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-v2-untokened");
    let (document, projection) = text_document(&h, &owner, "alpha beta\n").await;
    let mut untokened = request(&document, &projection, Vec::new());
    untokened.mentions = vec![owner.id.clone()];
    let error = h
        .service
        .create_comment(&owner, untokened, &trace())
        .await
        .expect_err("refused");
    assert!(matches!(error, DocumentsError::InvalidComment { .. }));
}
