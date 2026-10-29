use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{DeliveryState, MarkSeen};

#[test]
fn delivery_states_round_trip_through_their_string_form() {
    for state in [
        DeliveryState::Pending,
        DeliveryState::Delivered,
        DeliveryState::Held,
    ] {
        let text = state.to_string();
        assert_eq!(text, state.as_str());
        assert_eq!(text.parse::<DeliveryState>().expect("parses"), state);
        assert_eq!(
            serde_json::to_value(state).expect("serialises"),
            serde_json::Value::String(text)
        );
    }
}

#[test]
fn an_unknown_delivery_state_is_refused() {
    let error = "sent".parse::<DeliveryState>().expect_err("not a state");
    assert_eq!(error.field, "delivery_state");
    assert_eq!(error.value, "sent");
}

#[test]
fn a_seen_request_carries_only_ids() {
    let id = NotificationId::generate();
    let parsed: MarkSeen =
        serde_json::from_value(serde_json::json!({ "ids": [id.as_str()] })).expect("parses");
    assert_eq!(parsed.ids, vec![id]);
    assert!(
        serde_json::from_value::<MarkSeen>(serde_json::json!({})).is_err(),
        "ids are required"
    );
}
