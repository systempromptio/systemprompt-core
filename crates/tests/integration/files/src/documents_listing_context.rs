//! The `context` listing scope: the documents a conversation touched through
//! a revision or a comment, ranked and paged like every other scope, and
//! only while the caller can still see them.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::{
    CommentAnchor, GrantTarget, ListScope, ListSort, ListedDocument,
};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentOrigin, CommentPolicy, CommentRequest, CommitRequest, ListFolderRequest, ShareRequest,
};
use systemprompt_identifiers::{ContextId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};
use uuid::Uuid;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace, unique,
};

async fn created(
    h: &Harness,
    owner: &AuthenticatedUser,
    name: &str,
    context: Option<&ContextId>,
) -> DocumentId {
    let mut request = create("blank-text", None);
    request.name = name.to_owned();
    request.ops = vec![EditOp::SetText {
        text: "hello\n".to_owned(),
    }];
    request.author.attribution.context_id = context.cloned();
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

async fn share_with(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    reader: &AuthenticatedUser,
    level: AccessLevel,
) -> Uuid {
    h.service
        .share(
            owner,
            ShareRequest {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: reader.id.as_str().to_owned(),
                subject_display: None,
                level,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share")
        .id
}

async fn comment_in(
    h: &Harness,
    who: &AuthenticatedUser,
    document: &DocumentId,
    context: &ContextId,
) {
    let (_, projection) = h
        .service
        .project(who, document, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|n| n.text.as_deref() == Some("hello"))
        .expect("line")
        .id
        .clone();
    let request = CommentRequest {
        range: None,
        mentions: Vec::new(),
        document: document.clone(),
        anchor: Some(CommentAnchor {
            node,
            find: None,
            nth: None,
            cells: None,
        }),
        body: "from the conversation".to_owned(),
        parent_id: None,
        mention_options: vec![],
    };
    let origin = CommentOrigin {
        context_id: Some(context.clone()),
        ..CommentOrigin::default()
    };
    h.service
        .create_comment_from(who, request, &origin, &trace())
        .await
        .expect("comment");
}

async fn edit_in(h: &Harness, who: &AuthenticatedUser, document: &DocumentId, context: &ContextId) {
    let base = h
        .service
        .get(who, document, &trace())
        .await
        .expect("get")
        .head
        .map(|head| head.id);
    let mut author = api_author();
    author.attribution.context_id = Some(context.clone());
    h.service
        .commit(
            who,
            CommitRequest {
                document_id: document.clone(),
                base_revision: base,
                body: CommitBody::Ops(vec![EditOp::SetText {
                    text: "changed\n".to_owned(),
                }]),
                author,
                message: None,
                lock_holder: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("edit");
}

fn in_context(context: Option<&ContextId>, limit: i64) -> ListFolderRequest {
    ListFolderRequest {
        scope: ListScope::Context,
        folder: None,
        context_id: context.cloned(),
        query: None,
        sort: ListSort::Name,
        cursor: None,
        limit,
    }
}

fn ids(listed: &[ListedDocument]) -> Vec<DocumentId> {
    listed.iter().map(|l| l.document.id.clone()).collect()
}

#[tokio::test]
async fn a_context_lists_what_its_revisions_and_comments_touched() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let reader = person("context-reader");
    let owner = person("context-owner");
    let context = ContextId::generate();
    let elsewhere = ContextId::generate();

    let written = created(&h, &reader, &unique("a-written"), Some(&context)).await;
    let commented = created(&h, &owner, &unique("b-commented"), None).await;
    share_with(&h, &owner, &commented, &reader, AccessLevel::Comment).await;
    comment_in(&h, &reader, &commented, &context).await;
    let revoked = created(&h, &owner, &unique("c-revoked"), None).await;
    let grant = share_with(&h, &owner, &revoked, &reader, AccessLevel::Edit).await;
    edit_in(&h, &reader, &revoked, &context).await;
    let untouched = created(&h, &reader, &unique("d-untouched"), None).await;
    let other = created(&h, &reader, &unique("e-other"), Some(&elsewhere)).await;

    let before = h
        .service
        .list_folder(&reader, in_context(Some(&context), 50), &trace())
        .await
        .expect("list");
    assert_eq!(
        ids(&before.documents),
        vec![written.clone(), commented.clone(), revoked.clone()]
    );
    assert!(before.folders.is_empty());
    let access: Vec<AccessLevel> = before.documents.iter().map(|l| l.my_access).collect();
    assert_eq!(
        access,
        vec![AccessLevel::Own, AccessLevel::Comment, AccessLevel::Edit]
    );
    assert!(before.documents.iter().all(|l| l.touched_at.is_some()));

    h.service
        .revoke(
            &owner,
            &GrantTarget::Document(revoked.clone()),
            grant,
            &trace(),
        )
        .await
        .expect("revoke");
    let after = h
        .service
        .list_folder(&reader, in_context(Some(&context), 50), &trace())
        .await
        .expect("list");
    let seen = ids(&after.documents);
    assert_eq!(seen, vec![written, commented]);
    assert!(!seen.contains(&untouched) && !seen.contains(&other));

    let stranger = person("context-stranger");
    let theirs = h
        .service
        .list_folder(&stranger, in_context(Some(&context), 50), &trace())
        .await
        .expect("list");
    assert!(theirs.documents.is_empty(), "nothing the caller cannot see");
}

#[tokio::test]
async fn a_context_listing_pages_by_cursor() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let reader = person("context-pager");
    let context = ContextId::generate();
    let mut expected = Vec::new();
    for name in ["a", "b", "c"] {
        expected.push(created(&h, &reader, &unique(name), Some(&context)).await);
    }
    let mut seen = Vec::new();
    let mut request = in_context(Some(&context), 2);
    loop {
        let page = h
            .service
            .list_folder(&reader, request.clone(), &trace())
            .await
            .expect("page");
        assert!(page.documents.len() <= 2);
        seen.extend(ids(&page.documents));
        match page.next_cursor {
            Some(cursor) => request.cursor = Some(cursor),
            None => break,
        }
    }
    assert_eq!(seen, expected);
}

#[tokio::test]
async fn a_context_listing_without_a_context_is_invalid() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let reader = person("context-missing");
    let refused = h
        .service
        .list_folder(&reader, in_context(None, 50), &trace())
        .await
        .expect_err("no context");
    assert!(
        matches!(refused, DocumentsError::InvalidParams { .. }),
        "{refused:?}"
    );
}
