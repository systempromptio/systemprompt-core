use systemprompt_identifiers::{DocumentCommentId, DocumentId, UserId};
use systemprompt_models::notifications::{NotificationKind, SubjectRef};
use systemprompt_notifications::pipeline::ActionPromptBuilder;

use crate::fixtures::{event, stream_id};

const DOC: &str = "11111111-2222-4333-8444-555555555555";
const COMMENT: &str = "66666666-7777-4888-9999-aaaaaaaaaaaa";

#[test]
fn a_mention_prompt_links_the_comment_and_names_the_tools() {
    let mention = event(NotificationKind::MENTIONED).with_subjects([
        SubjectRef::new("document", DOC),
        SubjectRef::new("comment", COMMENT),
    ]);
    let action = ActionPromptBuilder::new(&mention).build();
    assert_eq!(
        action.open_url.as_deref(),
        Some(format!("/admin/drive/{DOC}?comment={COMMENT}").as_str())
    );
    assert_eq!(action.document_id, Some(DocumentId::new(DOC)));
    assert_eq!(action.comment_id, Some(DocumentCommentId::new(COMMENT)));
    assert_eq!(action.actor, UserId::new("actor"));
    assert_eq!(action.kind.as_str(), NotificationKind::MENTIONED);
    assert!(
        action
            .prompt
            .starts_with("actor mentioned you in a comment on document")
    );
    assert!(action.prompt.contains(DOC));
    assert!(action.prompt.contains("`documents_comment`"));
    assert!(action.prompt.contains("`documents_inbox`"));
}

#[test]
fn a_share_prompt_links_the_document() {
    let share = event(NotificationKind::SHARED).with_subject(SubjectRef::new("document", DOC));
    let action = ActionPromptBuilder::new(&share).build();
    assert_eq!(
        action.open_url.as_deref(),
        Some(format!("/admin/drive/{DOC}").as_str())
    );
    assert_eq!(action.comment_id, None);
    assert!(
        action
            .prompt
            .contains(&format!("shared document {DOC} with you"))
    );
    assert!(action.prompt.contains("`documents_read`"));
}

#[test]
fn a_stream_prompt_names_the_stream_and_has_no_link() {
    let published = event(NotificationKind::STREAM_PUBLISHED).with_stream(stream_id("news"));
    let action = ActionPromptBuilder::new(&published).build();
    assert_eq!(action.open_url, None);
    assert!(action.prompt.starts_with("actor published to stream news."));
}

#[test]
fn an_extension_kind_names_its_first_subject() {
    let custom = event("deal.stage_changed").with_subject(SubjectRef::new("deal", "d-42"));
    let action = ActionPromptBuilder::new(&custom).build();
    assert_eq!(action.open_url, None);
    assert!(
        action
            .prompt
            .starts_with("actor raised deal.stage_changed on deal:d-42.")
    );
}

#[test]
fn a_malformed_document_id_yields_no_link() {
    let mention =
        event(NotificationKind::MENTIONED).with_subject(SubjectRef::new("document", "not-a-uuid"));
    let action = ActionPromptBuilder::new(&mention).build();
    assert_eq!(action.open_url, None);
    assert_eq!(action.document_id, None);
    assert!(action.prompt.contains("document (unknown)"));
}
