use std::sync::Arc;

use systemprompt_extension::{ResourceVisibility, Visibility};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::AccessLevel;
use systemprompt_models::notifications::SubjectRef;
use systemprompt_runtime::documents::mcp::{ShareHandler, ShareInput};
use systemprompt_runtime::notifications::DocumentVisibility;

use crate::documents_mcp_rig::rig;
use crate::documents_mcp_support::{create_text, ctx_for, exec, registered};

fn grant(document: &DocumentId, user: &UserId, level: AccessLevel) -> ShareInput {
    ShareInput {
        document_id: Some(document.clone()),
        folder_id: None,
        subject_kind: Some("user".to_owned()),
        subject_value: Some(user.as_str().to_owned()),
        level: Some(level),
        valid_until: None,
        transfer_to: None,
        revoke_all: false,
        message: None,
    }
}

const fn visible(excerpt_allowed: bool) -> Visibility {
    Visibility::Visible { excerpt_allowed }
}

#[tokio::test]
async fn a_document_is_visible_by_level_and_its_excerpt_from_comment_up() {
    let r = rig(false).await;
    let owner = registered("vis-owner").await;
    let viewer = registered("vis-viewer").await;
    let commenter = registered("vis-commenter").await;
    let stranger = registered("vis-stranger").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&r.tools, &ctx, "body\n").await;
    let share = ShareHandler { tools: &r.tools };
    for (user, level) in [
        (&viewer, AccessLevel::View),
        (&commenter, AccessLevel::Comment),
    ] {
        share
            .handle(grant(&document, &user.id, level), &ctx, &exec())
            .await
            .expect("share");
    }
    let visibility = DocumentVisibility::new(
        Arc::clone(r.runtime.access()),
        Arc::clone(r.runtime.repositories()),
    );
    let subject = SubjectRef::new("document", document.as_str());

    assert_eq!(visibility.kinds(), &["document", "folder", "comment"]);
    assert_eq!(
        visibility.visible(&subject, &owner.id).await.unwrap(),
        visible(true)
    );
    assert_eq!(
        visibility.visible(&subject, &viewer.id).await.unwrap(),
        visible(false)
    );
    assert_eq!(
        visibility.visible(&subject, &stranger.id).await.unwrap(),
        Visibility::Hidden
    );

    let readers = [
        owner.id.clone(),
        viewer.id.clone(),
        commenter.id.clone(),
        stranger.id.clone(),
    ];
    assert_eq!(
        visibility.visible_many(&subject, &readers).await.unwrap(),
        vec![
            visible(true),
            visible(false),
            visible(true),
            Visibility::Hidden
        ]
    );
}

#[tokio::test]
async fn missing_documents_folders_and_unknown_comments_are_hidden() {
    let r = rig(false).await;
    let owner = registered("vis-hidden").await;
    let visibility = DocumentVisibility::new(
        Arc::clone(r.runtime.access()),
        Arc::clone(r.runtime.repositories()),
    );
    let readers = [owner.id.clone(), UserId::new("someone-else")];

    for subject in [
        SubjectRef::new("document", DocumentId::generate().as_str()),
        SubjectRef::new("folder", "folder-1"),
        SubjectRef::new("comment", uuid::Uuid::new_v4().to_string()),
        SubjectRef::new("other", "x"),
    ] {
        assert_eq!(
            visibility.visible(&subject, &owner.id).await.unwrap(),
            Visibility::Hidden,
            "{subject:?}"
        );
        assert_eq!(
            visibility.visible_many(&subject, &readers).await.unwrap(),
            vec![Visibility::Hidden, Visibility::Hidden],
            "{subject:?}"
        );
    }
}
