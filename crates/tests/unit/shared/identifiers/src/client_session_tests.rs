use systemprompt_identifiers::ClientSessionId;

const CLAUDE_CODE_SHAPE: &str = "user_1f8e6b2d9c4a7e0f1b3d5a7c9e2f4b6d8a0c2e4f6b8d0a2c4e6f8a0b2c4d6e8f_account_3c9b1d2e-7f4a-4b6c-9d8e-0f1a2b3c4d5e_session_9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f";

#[test]
fn parses_the_session_suffix_of_a_claude_code_user_id() {
    let id = ClientSessionId::from_metadata_user_id(CLAUDE_CODE_SHAPE).expect("session suffix");
    assert_eq!(id.as_str(), "9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f");
}

#[test]
fn normalises_the_suffix_to_lowercase_hyphenated_form() {
    let id = ClientSessionId::from_metadata_user_id(
        "user_x_session_9D2C4E6F-1A3B-4C5D-8E7F-0A1B2C3D4E5F",
    )
    .expect("uppercase uuid is still a uuid");
    assert_eq!(id.as_str(), "9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f");
}

#[test]
fn none_without_a_session_segment() {
    assert!(ClientSessionId::from_metadata_user_id("user-abc").is_none());
    assert!(ClientSessionId::from_metadata_user_id("user_x_account_y").is_none());
}

#[test]
fn none_when_the_suffix_is_not_a_uuid() {
    assert!(ClientSessionId::from_metadata_user_id("user_x_session_not-a-uuid").is_none());
    assert!(ClientSessionId::from_metadata_user_id("user_x_session_").is_none());
}

#[test]
fn the_last_session_segment_wins() {
    let id = ClientSessionId::from_metadata_user_id(
        "user_session_abc_session_9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f",
    )
    .expect("last segment");
    assert_eq!(id.as_str(), "9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f");
}

#[test]
fn try_new_accepts_only_lowercase_hyphenated_uuids() {
    assert!(ClientSessionId::try_new("9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f").is_ok());
    assert!(ClientSessionId::try_new("9D2C4E6F-1A3B-4C5D-8E7F-0A1B2C3D4E5F").is_err());
    assert!(ClientSessionId::try_new("sess_9d2c4e6f-1a3b-4c5d-8e7f-0a1b2c3d4e5f").is_err());
    assert!(ClientSessionId::try_new("").is_err());
}
