use systemprompt_identifiers::{EventOutboxId, UserId};
use systemprompt_models::notifications::{DigestWindow, SubjectRef};
use systemprompt_notifications::pipeline::coalesce::{coalesce_key, digest_release};

use crate::fixtures::{at, event, stream_id};

fn mention_at(secs: i64) -> systemprompt_models::notifications::NotificationEvent {
    let mut mention = event("document.comment.mentioned");
    mention.occurred_at = at(secs);
    mention.subjects = vec![
        SubjectRef::new("document", "doc-1"),
        SubjectRef::new("comment", "c-1"),
    ];
    mention
}

#[test]
fn the_key_names_the_kind_stream_primary_subject_actor_and_window_bucket() {
    let outbox = EventOutboxId::new("outbox-1");
    let key = coalesce_key(&mention_at(600), Some(300), &outbox);
    assert_eq!(key, "document.comment.mentioned:-:document:doc-1:actor:2");
    let streamed = mention_at(600).with_stream(stream_id("news"));
    assert_eq!(
        coalesce_key(&streamed, Some(300), &outbox),
        "document.comment.mentioned:news:document:doc-1:actor:2"
    );
}

#[test]
fn two_actors_on_one_subject_in_one_window_never_share_a_key() {
    let outbox = EventOutboxId::new("outbox-1");
    let mut other = mention_at(600);
    other.actor = UserId::new("someone-else");
    assert_ne!(
        coalesce_key(&mention_at(600), Some(300), &outbox),
        coalesce_key(&other, Some(300), &outbox)
    );
}

#[test]
fn the_same_subject_on_two_streams_never_shares_a_key() {
    let outbox = EventOutboxId::new("outbox-1");
    let news = mention_at(600).with_stream(stream_id("news"));
    let ops = mention_at(600).with_stream(stream_id("ops"));
    assert_ne!(
        coalesce_key(&news, Some(300), &outbox),
        coalesce_key(&ops, Some(300), &outbox)
    );
}

#[test]
fn events_in_one_window_share_a_key_and_the_next_window_does_not() {
    let outbox = EventOutboxId::new("outbox-1");
    let first = coalesce_key(&mention_at(600), Some(300), &outbox);
    let same = coalesce_key(&mention_at(899), Some(300), &EventOutboxId::new("outbox-2"));
    let next = coalesce_key(&mention_at(900), Some(300), &outbox);
    assert_eq!(first, same);
    assert_ne!(first, next);
}

#[test]
fn without_a_window_every_event_keeps_its_own_key() {
    let one = coalesce_key(&mention_at(600), None, &EventOutboxId::new("outbox-1"));
    let two = coalesce_key(&mention_at(600), None, &EventOutboxId::new("outbox-2"));
    assert_ne!(one, two);
    assert!(one.ends_with(":outbox-1"));
    let zero = coalesce_key(&mention_at(600), Some(0), &EventOutboxId::new("outbox-3"));
    assert!(zero.ends_with(":outbox-3"), "a zero window is no window");
}

#[test]
fn different_documents_never_share_a_key() {
    let outbox = EventOutboxId::new("outbox-1");
    let mut other = mention_at(600);
    other.subjects[0] = SubjectRef::new("document", "doc-2");
    assert_ne!(
        coalesce_key(&mention_at(600), Some(300), &outbox),
        coalesce_key(&other, Some(300), &outbox)
    );
}

#[test]
fn an_event_without_subjects_uses_placeholders() {
    let bare = event("deal.stage_changed");
    let key = coalesce_key(&bare, None, &EventOutboxId::new("o"));
    assert_eq!(key, "deal.stage_changed:-:-:-:actor:o");
}

#[test]
fn a_digest_holds_until_the_end_of_the_window_the_event_falls_in() {
    let hourly = Some(DigestWindow { every_secs: 3600 });
    assert_eq!(digest_release(&mention_at(1000), hourly), Some(at(3600)));
    assert_eq!(digest_release(&mention_at(3599), hourly), Some(at(3600)));
    assert_eq!(
        digest_release(&mention_at(3600), hourly),
        Some(at(7200)),
        "an event on the boundary opens the next window"
    );
}

#[test]
fn no_digest_or_a_zero_window_holds_nothing() {
    assert_eq!(digest_release(&mention_at(1000), None), None);
    assert_eq!(
        digest_release(&mention_at(1000), Some(DigestWindow { every_secs: 0 })),
        None
    );
}
