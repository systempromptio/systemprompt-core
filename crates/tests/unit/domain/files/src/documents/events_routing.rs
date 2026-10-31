//! Document events routed without a database: the live topic each event
//! reaches, the actor it is attributed to, and the errors a relay that
//! cannot reach its outbox reports.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_events::{EventBroadcasters, EventRouter, RELAY_LOST_EVENT};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::events::{DocumentEvents, live_topic};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, UserId};
use systemprompt_models::documents::{
    DocumentCollabUpdateEvent, DocumentLockEvent, DocumentPresenceEvent, PresenceSource,
    PresenceState,
};

const DOCUMENT: &str = "00000000-0000-4000-8000-00000000e001";
const SESSION: &str = "00000000-0000-4000-8000-00000000e002";

fn local_events() -> (DocumentEvents, Arc<EventBroadcasters>) {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let router = EventRouter::local_only(Arc::clone(&broadcasters));
    (DocumentEvents::new(Arc::new(router)), broadcasters)
}

fn unreachable_events() -> (DocumentEvents, Arc<EventBroadcasters>) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(200))
        .connect_lazy("postgres://closed:closed@127.0.0.1:1/closed")
        .expect("lazy pool");
    let broadcasters = Arc::new(EventBroadcasters::new());
    let router = EventRouter::with_outbox(
        pool,
        systemprompt_test_fixtures::unique_instance(),
        Arc::clone(&broadcasters),
    );
    (DocumentEvents::new(Arc::new(router)), broadcasters)
}

fn presence(user: &str) -> DocumentPresenceEvent {
    DocumentPresenceEvent {
        document_id: DocumentId::new(DOCUMENT),
        user_id: UserId::new(user),
        state: PresenceState::Viewing,
        source_kind: PresenceSource::Web,
        source_ref: "tab".to_owned(),
        cursor: None,
    }
}

fn unlocked() -> DocumentLockEvent {
    DocumentLockEvent {
        document_id: DocumentId::new(DOCUMENT),
        locked: false,
        holder_id: None,
        edit_session_id: None,
    }
}

fn collab_update() -> DocumentCollabUpdateEvent {
    DocumentCollabUpdateEvent {
        document_id: DocumentId::new(DOCUMENT),
        edit_session_id: DocumentEditSessionId::new(SESSION),
        seq: 7,
        update_b64: "AAEC".to_owned(),
        author_id: UserId::new("collab-author"),
    }
}

#[tokio::test]
async fn a_recipient_event_reaches_the_live_topic_attributed_to_its_actor() {
    let (events, broadcasters) = local_events();
    let topic = live_topic(&DocumentId::new(DOCUMENT));
    assert_eq!(topic, format!("document:{DOCUMENT}"));
    let mut live = broadcasters.topics.subscribe(&topic);
    events
        .presence(&[UserId::new("reader")], &presence("viewer"))
        .await
        .expect("local delivery");
    let event = live.try_recv().expect("topic event");
    assert_eq!(event.name, DocumentPresenceEvent::NAME);
    assert_eq!(event.value["user_id"], "viewer");
    assert!(event.resume_id.is_none());
}

#[tokio::test]
async fn an_event_naming_no_actor_and_no_recipient_skips_the_topic() {
    let (events, broadcasters) = local_events();
    let mut live = broadcasters
        .topics
        .subscribe(&live_topic(&DocumentId::new(DOCUMENT)));
    events.lock(&[], &unlocked()).await.expect("nothing to do");
    assert!(live.try_recv().is_err());
    events
        .lock(&[UserId::new("lock-watcher")], &unlocked())
        .await
        .expect("first recipient attributes the topic event");
    let event = live.try_recv().expect("topic event");
    assert_eq!(event.name, DocumentLockEvent::NAME);
    assert_eq!(event.value["locked"], false);
}

#[tokio::test]
async fn a_collab_update_carries_its_resume_token() {
    let (events, broadcasters) = local_events();
    let mut live = broadcasters
        .topics
        .subscribe(&live_topic(&DocumentId::new(DOCUMENT)));
    events
        .collab_update(&collab_update())
        .await
        .expect("local only");
    let event = live.try_recv().expect("update");
    assert_eq!(event.name, DocumentCollabUpdateEvent::NAME);
    assert_eq!(event.resume_id.as_deref(), Some(&*format!("{SESSION}:7")));
    assert!(live.try_recv().is_err(), "no relay-lost on success");
}

#[tokio::test]
async fn an_unrelayed_collab_update_errs_and_announces_the_gap() {
    let (events, broadcasters) = unreachable_events();
    let mut live = broadcasters
        .topics
        .subscribe(&live_topic(&DocumentId::new(DOCUMENT)));
    let refused = events
        .collab_update(&collab_update())
        .await
        .expect_err("relay cannot persist");
    assert!(matches!(
        refused,
        DocumentsError::Events { event, .. } if event == DocumentCollabUpdateEvent::NAME
    ));
    let update = live.try_recv().expect("delivered locally first");
    assert_eq!(update.name, DocumentCollabUpdateEvent::NAME);
    let gap = live.try_recv().expect("gap announced locally");
    assert_eq!(gap.name, RELAY_LOST_EVENT);
}

#[tokio::test]
async fn an_unrelayed_recipient_event_reports_the_relay_failure() {
    let (events, broadcasters) = unreachable_events();
    let mut live = broadcasters
        .topics
        .subscribe(&live_topic(&DocumentId::new(DOCUMENT)));
    let refused = events
        .presence(
            &[UserId::new("reader-a"), UserId::new("reader-b")],
            &presence("viewer"),
        )
        .await
        .expect_err("outbox unreachable");
    assert!(matches!(
        refused,
        DocumentsError::Events { event, .. } if event == DocumentPresenceEvent::NAME
    ));
    assert_eq!(
        live.try_recv().expect("topic delivered locally").name,
        DocumentPresenceEvent::NAME
    );
}

#[tokio::test]
async fn an_event_with_no_recipient_and_a_failing_relay_still_errs_only_on_topic_audience() {
    let (events, _broadcasters) = unreachable_events();
    events
        .lock(&[], &unlocked())
        .await
        .expect("no actor, no recipient: nothing relayed");
}
