//! `AgUiEvent` — the streaming envelope a client dispatches on.
//!
//! Three separate code paths must agree on one string per event:
//! `AgUiEventType::as_str`, `AgUiEvent::event_type`, and the serde tag emitted
//! by `rename_all = "SCREAMING_SNAKE_CASE"`. The sibling `agui` suite already
//! covers `as_str`, so a rename of the serde tagging would leave it green
//! while the wire broke — these assert the three against each other.

use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::agui::{AgUiEvent, AgUiEventBuilder, AgUiEventType};

fn events() -> Vec<AgUiEvent> {
    vec![
        AgUiEventBuilder::run_started(
            ContextId::generate(),
            TaskId::generate(),
            Some(serde_json::json!({"prompt": "hi"})),
        ),
        AgUiEventBuilder::run_error("boom".to_owned(), Some("E_BOOM".to_owned())),
        AgUiEventBuilder::step_started("planning"),
    ]
}

fn wire(event: &AgUiEvent) -> serde_json::Value {
    serde_json::to_value(event).expect("serialise event")
}

// Why: the tag is what a client switches on to decide how to read the rest of
// the frame. If it disagreed with `event_type`, code inside the process and
// code across the wire would classify the same event differently.
#[test]
fn the_serde_tag_agrees_with_the_event_type_for_every_event_built() {
    for event in events() {
        let json = wire(&event);
        let tag = json["type"].as_str().expect("every event carries a type");

        assert_eq!(
            tag,
            event.event_type().as_str(),
            "{event:?} serialises as {tag} but reports {}",
            event.event_type().as_str()
        );
    }
}

#[test]
fn the_tag_is_screaming_snake_case_as_the_protocol_specifies() {
    let started = wire(&AgUiEventBuilder::run_started(
        ContextId::generate(),
        TaskId::generate(),
        None,
    ));

    assert_eq!(started["type"], "RUN_STARTED");

    let errored = wire(&AgUiEventBuilder::run_error("boom".to_owned(), None));
    assert_eq!(errored["type"], "RUN_ERROR");
}

// Why: the payload is flattened into the frame, not nested under a key, and
// its fields are camelCase — the AG-UI protocol's spelling, not Rust's. A
// client reads `threadId` at the top level; nested or snake_cased, every field
// it wants is somewhere other than where it looks.
#[test]
fn the_payload_is_flattened_beside_the_tag_rather_than_nested() {
    let context = ContextId::generate();
    let task = TaskId::generate();
    let json = wire(&AgUiEventBuilder::run_started(
        context.clone(),
        task.clone(),
        None,
    ));

    assert!(
        json.get("payload").is_none(),
        "the payload must not appear as its own key: {json}"
    );
    assert_eq!(
        json["threadId"].as_str(),
        Some(context.as_str()),
        "payload fields sit beside the tag under their camelCase names: {json}"
    );
    assert_eq!(json["runId"].as_str(), Some(task.as_str()));
    assert!(
        json.get("thread_id").is_none(),
        "the Rust field name must not appear alongside the wire name: {json}"
    );
}

// Why: every frame is timestamped, and a consumer orders frames by it. An
// event without one cannot be placed in the stream.
#[test]
fn every_event_carries_a_timestamp() {
    for event in events() {
        let json = wire(&event);
        assert!(
            json["timestamp"].as_str().is_some(),
            "an event with no timestamp cannot be ordered: {json}"
        );
    }
}

// Why: an absent optional stays absent. `input: null` reads as an input that
// was supplied and empty, rather than one that was never given.
#[test]
fn an_absent_optional_payload_field_is_omitted_rather_than_null() {
    let without = wire(&AgUiEventBuilder::run_started(
        ContextId::generate(),
        TaskId::generate(),
        None,
    ));
    assert!(
        without.get("input").is_none(),
        "an absent input must be omitted: {without}"
    );

    let with = wire(&AgUiEventBuilder::run_started(
        ContextId::generate(),
        TaskId::generate(),
        Some(serde_json::json!({"prompt": "hi"})),
    ));
    assert_eq!(with["input"]["prompt"], "hi");
}

// Why: a tagged enum must read back as the variant it was written from, or a
// consumer that deserialises the stream loses events it cannot classify.
#[test]
fn an_event_reads_back_as_the_variant_it_was_written_from() {
    for event in events() {
        let json = wire(&event);
        let back: AgUiEvent =
            serde_json::from_value(json.clone()).unwrap_or_else(|e| panic!("{json}: {e}"));

        assert_eq!(
            back.event_type().as_str(),
            event.event_type().as_str(),
            "round trip changed the event type"
        );
    }
}

#[test]
fn an_error_event_carries_its_message_and_code() {
    let json = wire(&AgUiEventBuilder::run_error(
        "boom".to_owned(),
        Some("E_BOOM".to_owned()),
    ));

    assert_eq!(json["type"], AgUiEventType::RunError.as_str());
    assert_eq!(json["message"], "boom");
    assert_eq!(json["code"], "E_BOOM");
}
