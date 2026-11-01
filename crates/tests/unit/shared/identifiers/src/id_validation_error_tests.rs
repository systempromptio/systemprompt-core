use std::error::Error;

use systemprompt_identifiers::error::IdValidationError;

fn uuid_error() -> uuid::Error {
    uuid::Uuid::parse_str("not-a-uuid").expect_err("invalid uuid")
}

fn json_error() -> serde_json::Error {
    serde_json::from_str::<serde_json::Value>("{").expect_err("invalid json")
}

#[test]
fn json_errors_keep_their_source_and_type() {
    let err = IdValidationError::json("ContextId", json_error());
    assert!(err.to_string().starts_with("ContextId: "));
    assert!(err.source().is_some());
    assert!(matches!(
        err,
        IdValidationError::Json {
            id_type: "ContextId",
            ..
        }
    ));
}

#[test]
fn equality_compares_variant_type_and_message() {
    assert_eq!(
        IdValidationError::empty("UserId"),
        IdValidationError::empty("UserId")
    );
    assert_ne!(
        IdValidationError::empty("UserId"),
        IdValidationError::empty("TaskId")
    );
    assert_eq!(
        IdValidationError::invalid("UserId", "bad"),
        IdValidationError::invalid("UserId", "bad")
    );
    assert_ne!(
        IdValidationError::invalid("UserId", "bad"),
        IdValidationError::invalid("UserId", "worse")
    );
    assert_ne!(
        IdValidationError::empty("UserId"),
        IdValidationError::invalid("UserId", "")
    );
}

#[test]
fn equality_covers_sourced_variants() {
    assert_eq!(
        IdValidationError::uuid("TaskId", uuid_error()),
        IdValidationError::uuid("TaskId", uuid_error())
    );
    assert_ne!(
        IdValidationError::uuid("TaskId", uuid_error()),
        IdValidationError::uuid("ContextId", uuid_error())
    );
    assert_eq!(
        IdValidationError::json("TaskId", json_error()),
        IdValidationError::json("TaskId", json_error())
    );
    assert_ne!(
        IdValidationError::json("TaskId", json_error()),
        IdValidationError::uuid("TaskId", uuid_error())
    );
}

#[test]
fn a_cloned_json_error_stays_equal() {
    let err = IdValidationError::json("TraceId", json_error());
    assert_eq!(err.clone(), err);
}
