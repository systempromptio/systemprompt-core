use chrono::{DateTime, Utc};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::notifications::{RuleAction, RuleMatch};

use crate::harness::{Harness, mention};

fn window_start(window: i64) -> DateTime<Utc> {
    let now = Utc::now().timestamp();
    DateTime::from_timestamp(now - now.rem_euclid(window), 0).expect("timestamp")
}

#[tokio::test]
async fn mentions_on_one_document_inside_the_window_fold_into_one_row() {
    let h = Harness::new("notif_coalesce").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    h.declare_rule(
        RuleMatch {
            kinds: vec!["document.comment.*".to_owned()],
            ..RuleMatch::default()
        },
        RuleAction {
            coalesce_window_secs: Some(300),
            ..RuleAction::default()
        },
    )
    .await;
    let document = DocumentId::generate();
    let start = window_start(300);
    for offset in [10, 70, 130] {
        let at = start + chrono::Duration::seconds(offset);
        let (event, _) = mention(&author, &reader, &document, at);
        h.emit(&event).await;
    }

    let drained = h.drain().await;
    assert_eq!(drained.ids.len(), 3);
    let coalesced: u32 = drained.outcomes.iter().map(|o| o.coalesced).sum();
    assert_eq!(coalesced, 2);

    let rows = h.rows_for(&reader).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].coalesce_count, 3);
    assert_eq!(
        rows[0].first_event_at,
        start + chrono::Duration::seconds(10)
    );
    assert_eq!(
        rows[0].last_event_at,
        start + chrono::Duration::seconds(130)
    );
    h.finish().await;
}

#[tokio::test]
async fn without_a_window_each_mention_keeps_its_own_row() {
    let h = Harness::new("notif_no_coalesce").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    let document = DocumentId::generate();
    let start = window_start(300);
    for offset in [10, 20] {
        let (event, _) = mention(
            &author,
            &reader,
            &document,
            start + chrono::Duration::seconds(offset),
        );
        h.emit(&event).await;
    }
    h.drain().await;
    let rows = h.rows_for(&reader).await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.coalesce_count == 1));
    h.finish().await;
}

async fn coalescing_rule(h: &Harness) {
    h.declare_rule(
        RuleMatch {
            kinds: vec!["document.comment.*".to_owned()],
            ..RuleMatch::default()
        },
        RuleAction {
            coalesce_window_secs: Some(300),
            ..RuleAction::default()
        },
    )
    .await;
}

#[tokio::test]
async fn two_actors_on_one_document_never_fold_into_one_row_or_share_a_body() {
    let h = Harness::new("notif_coalesce_actors").await;
    let first = h.user("first", &[]).await;
    let second = h.user("second", &[]).await;
    let reader = h.user("reader", &[]).await;
    coalescing_rule(&h).await;
    let document = DocumentId::generate();
    let start = window_start(300);
    let (one, one_comment) = mention(
        &first,
        &reader,
        &document,
        start + chrono::Duration::seconds(10),
    );
    let (two, two_comment) = mention(
        &second,
        &reader,
        &document,
        start + chrono::Duration::seconds(20),
    );
    h.emit(&one.with_body("from the first actor")).await;
    h.emit(&two.with_body("from the second actor")).await;

    let drained = h.drain().await;
    let coalesced: u32 = drained.outcomes.iter().map(|o| o.coalesced).sum();
    assert_eq!(coalesced, 0, "different actors never share a coalesce key");

    let rows = h.rows_for(&reader).await;
    assert_eq!(rows.len(), 2, "{rows:?}");
    for (actor, body, comment) in [
        (&first, "from the first actor", &one_comment),
        (&second, "from the second actor", &two_comment),
    ] {
        let row = rows
            .iter()
            .find(|r| &r.actor == actor)
            .expect("a row per actor");
        assert_eq!(row.coalesce_count, 1);
        assert_eq!(row.body, body);
        assert_eq!(&row.action.actor, actor);
        assert_eq!(row.action.comment_id.as_ref(), Some(comment));
    }
    h.finish().await;
}

#[tokio::test]
async fn a_fold_carries_the_latest_events_body_action_and_subjects_together() {
    let h = Harness::new("notif_coalesce_fold").await;
    let author = h.user("author", &[]).await;
    let reader = h.user("reader", &[]).await;
    coalescing_rule(&h).await;
    let document = DocumentId::generate();
    let start = window_start(300);
    let (earlier, _) = mention(
        &author,
        &reader,
        &document,
        start + chrono::Duration::seconds(10),
    );
    let (later, later_comment) = mention(
        &author,
        &reader,
        &document,
        start + chrono::Duration::seconds(40),
    );
    h.emit(&earlier.with_body("first draft")).await;
    h.emit(&later.with_body("second draft")).await;
    h.drain().await;

    let rows = h.rows_for(&reader).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.coalesce_count, 2);
    assert_eq!(row.body, "second draft");
    assert_eq!(row.action.comment_id.as_ref(), Some(&later_comment));
    assert!(
        row.subjects
            .iter()
            .any(|s| s.kind == "comment" && s.id == later_comment.as_str()),
        "{:?}",
        row.subjects
    );
    assert_eq!(row.last_event_at, start + chrono::Duration::seconds(40));
    h.finish().await;
}
