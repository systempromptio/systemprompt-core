use std::str::FromStr;
use systemprompt_traits::RequestStatus;

#[test]
fn display_and_from_str_round_trip_every_variant() {
    for (variant, text) in [
        (RequestStatus::Pending, "pending"),
        (RequestStatus::Completed, "completed"),
        (RequestStatus::Failed, "failed"),
        (RequestStatus::Rejected, "rejected"),
    ] {
        assert_eq!(variant.to_string(), text);
        assert_eq!(variant.as_str(), text);
        assert_eq!(RequestStatus::from_str(text).unwrap(), variant);
    }
}

#[test]
fn unknown_values_are_errors_not_a_catch_all_variant() {
    for bad in ["other", "Completed", "", "bogus"] {
        assert_eq!(RequestStatus::from_str(bad).unwrap_err().0, bad);
    }
}

#[test]
fn default_is_pending_and_only_terminal_states_are_settled() {
    assert_eq!(RequestStatus::default(), RequestStatus::Pending);
    assert!(!RequestStatus::Pending.is_settled());
    assert!(RequestStatus::Completed.is_settled());
    assert!(RequestStatus::Failed.is_settled());
    assert!(RequestStatus::Rejected.is_settled());
}

#[test]
fn serde_uses_the_persisted_strings() {
    assert_eq!(
        serde_json::to_string(&RequestStatus::Rejected).unwrap(),
        "\"rejected\""
    );
    let parsed: RequestStatus = serde_json::from_str("\"failed\"").unwrap();
    assert_eq!(parsed, RequestStatus::Failed);
}
