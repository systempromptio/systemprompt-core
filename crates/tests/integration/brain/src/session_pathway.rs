//! The session pathway is scaffolding: a `session.turn.closed` event is
//! ignored before the filters while `capture.sources` omits `session`, and
//! with the source on but no filter registered it records `no_filter` and
//! extracts nothing.

use chrono::Utc;
use systemprompt_identifiers::{AiRequestId, SessionId, UserId};
use systemprompt_manifest::profile::CaptureSource;
use systemprompt_models::brain::{BrainEvent, BrainEventKind, SessionTurnClosed};

use crate::harness::{Harness, Setup};

fn turn(user: &UserId) -> BrainEvent {
    let payload = SessionTurnClosed {
        user_id: user.clone(),
        session_id: SessionId::generate(),
        ai_request_id: AiRequestId::generate(),
        gateway_conversation_id: None,
        context_id: None,
        user_text: "what is acme's price?".to_owned(),
        assistant_text: "acme pays 1200 in emea".to_owned(),
        model: "claude-test".to_owned(),
        client: "claude-code".to_owned(),
    };
    BrainEvent::new(
        BrainEventKind::session_turn_closed(),
        user.clone(),
        Utc::now(),
    )
    .with_payload(serde_json::to_value(payload).expect("payload"))
}

#[tokio::test]
async fn by_default_a_session_turn_is_ignored_before_the_filters() {
    let h = Harness::new("brain_session_default").await;
    let user = h.user("user", &[]).await;
    assert!(!h.profile.capture.session_capture_on());

    let id = h.emit(&turn(&user)).await;
    h.drain().await;

    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1 \
             AND outcome = 'ignored' AND filter = 'capture.sources'",
            id.as_str()
        )
        .await,
        1
    );
    assert_eq!(h.total("brain_facts").await, 0);
    assert_eq!(h.total("brain_extractions").await, 0);
    assert_eq!(h.model.calls(), 0);
    h.finish().await;
}

#[tokio::test]
async fn with_the_source_on_and_no_filter_nothing_is_extracted() {
    let mut setup = Setup {
        gather: None,
        ..Setup::default()
    };
    setup.profile.capture.sources.push(CaptureSource::Session);
    setup.profile.capture.session.enabled = true;
    let h = Harness::with("brain_session_no_filter", setup).await;
    let user = h.user("user", &[]).await;
    assert!(h.profile.capture.session_capture_on());

    let id = h.emit(&turn(&user)).await;
    h.drain().await;

    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1 \
             AND outcome = 'no_filter' AND filter IS NULL",
            id.as_str()
        )
        .await,
        1
    );
    assert_eq!(h.total("brain_facts").await, 0);
    assert_eq!(h.total("brain_extractions").await, 0);
    assert_eq!(h.model.calls(), 0);
    h.finish().await;
}
