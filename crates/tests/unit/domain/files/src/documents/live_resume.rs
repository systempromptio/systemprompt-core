use systemprompt_events::TopicEvent;
use systemprompt_files::documents::events::live_topic;
use systemprompt_files::documents::live::{LiveCursor, LiveDelivery, ResumeToken};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentId};

const SESSION: &str = "0b9a3f8e-6c1d-4f5e-9a7b-2c3d4e5f6a7b";

#[test]
fn a_session_and_seq_parse_as_a_resume_token() {
    let token = ResumeToken::parse(&format!("{SESSION}:42")).expect("token");
    assert_eq!(token.session.as_str(), SESSION);
    assert_eq!(token.seq, 42);
}

#[test]
fn anything_else_is_no_resume_token() {
    assert!(ResumeToken::parse("").is_none());
    assert!(ResumeToken::parse(SESSION).is_none());
    assert!(ResumeToken::parse(&format!("{SESSION}:next")).is_none());
    assert!(ResumeToken::parse("not-a-session:3").is_none());
}

#[test]
fn the_live_topic_names_the_document() {
    let document = DocumentId::new("5f0c1e2d-3b4a-4c5d-8e9f-0a1b2c3d4e5f");
    assert_eq!(
        live_topic(&document),
        "document:5f0c1e2d-3b4a-4c5d-8e9f-0a1b2c3d4e5f"
    );
}

fn update(seq: i64) -> TopicEvent {
    TopicEvent {
        topic: "document:x".to_owned(),
        name: "document.collab.update".to_owned(),
        resume_id: Some(format!("{SESSION}:{seq}")),
        value: serde_json::Value::Null,
    }
}

fn id(seq: i64) -> LiveDelivery {
    LiveDelivery::Deliver(Some(format!("{SESSION}:{seq}")))
}

#[test]
fn an_out_of_order_update_does_not_advance_the_resume_id_past_a_gap() {
    let mut cursor = LiveCursor::at(Some(DocumentEditSessionId::new(SESSION)), 3);
    assert_eq!(cursor.live(&update(5)), id(3));
    assert_eq!(cursor.through(), 3);
    assert_eq!(cursor.live(&update(4)), id(5));
    assert_eq!(cursor.live(&update(6)), id(6));
}

#[test]
fn a_resumed_channel_skips_what_it_replayed_and_resumes_from_the_contiguous_max() {
    let token = ResumeToken::parse(&format!("{SESSION}:2")).expect("token");
    let mut cursor = LiveCursor::resumed(&token);
    let session = DocumentEditSessionId::new(SESSION);
    assert_eq!(cursor.replayed(&session, 3), format!("{SESSION}:3"));
    assert_eq!(cursor.replayed(&session, 4), format!("{SESSION}:4"));
    assert_eq!(cursor.live(&update(4)), LiveDelivery::Skip);
    assert_eq!(cursor.live(&update(3)), LiveDelivery::Skip);
    assert_eq!(cursor.live(&update(5)), id(5));
}

#[test]
fn events_other_than_updates_carry_no_resume_id() {
    let mut cursor = LiveCursor::at(None, 0);
    let event = TopicEvent {
        topic: "document:x".to_owned(),
        name: "document.awareness".to_owned(),
        resume_id: None,
        value: serde_json::Value::Null,
    };
    assert_eq!(cursor.live(&event), LiveDelivery::Deliver(None));
}

#[test]
fn a_new_session_restarts_the_contiguous_count() {
    let mut cursor = LiveCursor::at(Some(DocumentEditSessionId::new(SESSION)), 9);
    let other = "1c2d3e4f-5a6b-4c7d-8e9f-0a1b2c3d4e5f";
    let event = TopicEvent {
        resume_id: Some(format!("{other}:1")),
        ..update(1)
    };
    assert_eq!(
        cursor.live(&event),
        LiveDelivery::Deliver(Some(format!("{other}:1")))
    );
}
