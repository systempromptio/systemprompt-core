use chrono::{TimeZone, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{NotificationEvent, NotificationKind};

fn event() -> NotificationEvent {
    NotificationEvent::new(
        NotificationKind::new("workflow.decision.requested").expect("kind"),
        UserId::new("admin"),
        Utc.timestamp_opt(1_000_000, 0).single().expect("timestamp"),
    )
}

#[test]
fn an_event_stored_before_the_system_flag_decodes_as_a_persons() {
    let mut stored = serde_json::to_value(event()).expect("serialise");
    stored
        .as_object_mut()
        .expect("object")
        .remove("actor_is_system")
        .expect("the field is serialised");

    let decoded: NotificationEvent = serde_json::from_value(stored).expect("decode");
    assert!(!decoded.actor_is_system);
    assert_eq!(decoded, event());
}

#[test]
fn the_system_flag_survives_the_outbox() {
    let system = event().with_actor_is_system(true);
    let decoded: NotificationEvent =
        serde_json::from_value(serde_json::to_value(&system).expect("serialise")).expect("decode");
    assert!(decoded.actor_is_system);
}
