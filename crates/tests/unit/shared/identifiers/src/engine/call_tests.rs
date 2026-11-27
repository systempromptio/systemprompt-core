use std::str::FromStr;

use systemprompt_identifiers::engine::{CallId, IdError};

#[test]
fn keeps_the_model_value_verbatim() {
    let id = CallId::try_new("toolu_01 A/b+c").unwrap();
    assert_eq!(id.as_str(), "toolu_01 A/b+c");
    assert_eq!(id.to_string(), "toolu_01 A/b+c");
    assert_eq!(String::from(id), "toolu_01 A/b+c");
}

#[test]
fn rejects_an_empty_value() {
    assert_eq!(
        CallId::try_new("").unwrap_err(),
        IdError {
            kind: "call",
            value: String::new()
        }
    );
    assert!(CallId::from_str("").is_err());
    assert!(CallId::try_from(String::new()).is_err());
}

#[test]
fn accepts_any_non_empty_value() {
    assert_eq!(CallId::try_new(" ").unwrap().as_str(), " ");
    assert_eq!(
        CallId::try_from(String::from("call_1")).unwrap().as_str(),
        "call_1"
    );
}

#[test]
fn serde_round_trips_and_rejects_empty() {
    let id = CallId::try_new("call_abc").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"call_abc\"");
    assert_eq!(serde_json::from_str::<CallId>(&json).unwrap(), id);
    assert!(serde_json::from_str::<CallId>("\"\"").is_err());
}
