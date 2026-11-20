//! The actor is never their own recipient, except where the event names them
//! directly at `agent` attention: a hand-off to their own agent lands in
//! their feed at `agent`, and a self-mention at any other attention is
//! dropped.

use chrono::Utc;
use systemprompt_identifiers::{DocumentCommentId, DocumentId, UserId};
use systemprompt_models::notifications::{
    Attention, NotificationEvent, NotificationKind, SubjectRef,
};

use crate::harness::Harness;

fn self_mention(author: &UserId, attention: Option<Attention>) -> NotificationEvent {
    NotificationEvent::new(
        NotificationKind::new(NotificationKind::MENTIONED).expect("kind"),
        author.clone(),
        Utc::now(),
    )
    .with_subjects([
        SubjectRef::new("document", DocumentId::generate().as_str()),
        SubjectRef::new("comment", DocumentCommentId::generate().as_str()),
    ])
    .with_direct_recipient(author.clone(), attention)
    .with_body("pick this up")
}

#[tokio::test]
async fn a_self_mention_at_agent_attention_reaches_the_actors_feed() {
    let h = Harness::new("notif_self_agent").await;
    let author = h.user("author", &[]).await;
    h.emit(&self_mention(&author, Some(Attention::Agent))).await;
    h.drain().await;

    let rows = h.rows_for(&author).await;
    assert_eq!(rows.len(), 1, "the hand-off is delivered");
    assert_eq!(rows[0].attention, Attention::Agent);
    assert_eq!(rows[0].actor, author);
    h.finish().await;
}

#[tokio::test]
async fn a_self_mention_at_any_other_attention_is_dropped() {
    let h = Harness::new("notif_self_quiet").await;
    let author = h.user("author", &[]).await;
    for attention in [Some(Attention::Human), Some(Attention::Silent), None] {
        h.emit(&self_mention(&author, attention)).await;
    }
    h.drain().await;

    assert!(
        h.rows_for(&author).await.is_empty(),
        "the actor is not their own recipient"
    );
    h.finish().await;
}
