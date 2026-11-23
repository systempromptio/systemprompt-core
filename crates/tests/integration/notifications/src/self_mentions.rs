//! The actor is never their own recipient, except where the event names them
//! directly at `agent` attention: a hand-off to their own agent lands in
//! their feed at `agent`, and a self-mention at any other attention is
//! dropped. An event the platform produced (`actor_is_system`) reaches the
//! account it is attributed to like any other recipient.

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

#[tokio::test]
async fn a_system_event_reaches_the_account_it_is_attributed_to() {
    let h = Harness::new("notif_self_system").await;
    let admin = h.user("admin", &[]).await;
    let other = h.user("other", &[]).await;
    h.emit(
        &self_mention(&admin, Some(Attention::Human))
            .with_direct_recipient(other.clone(), Some(Attention::Human))
            .with_actor_is_system(true),
    )
    .await;
    h.drain().await;

    let rows = h.rows_for(&admin).await;
    assert_eq!(rows.len(), 1, "the platform's event reaches its actor");
    assert_eq!(rows[0].attention, Attention::Human);
    assert_eq!(rows[0].actor, admin);
    assert_eq!(h.rows_for(&other).await.len(), 1);
    h.finish().await;
}

#[tokio::test]
async fn the_same_event_without_the_system_flag_drops_only_the_actor() {
    let h = Harness::new("notif_self_person").await;
    let admin = h.user("admin", &[]).await;
    let other = h.user("other", &[]).await;
    h.emit(
        &self_mention(&admin, Some(Attention::Human))
            .with_direct_recipient(other.clone(), Some(Attention::Human)),
    )
    .await;
    h.drain().await;

    assert!(
        h.rows_for(&admin).await.is_empty(),
        "the actor is not their own recipient"
    );
    assert_eq!(h.rows_for(&other).await.len(), 1);
    h.finish().await;
}
