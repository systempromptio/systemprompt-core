use systemprompt_identifiers::SessionId;

#[test]
fn minted_bot_session_reads_as_bot() {
    assert!(SessionId::bot().is_bot());
}

#[test]
fn generated_and_untracked_sessions_are_not_bots() {
    assert!(!SessionId::generate().is_bot());
    assert!(!SessionId::untracked().is_bot());
    assert!(!SessionId::system().is_bot());
}

#[test]
fn bot_prefix_is_matched_at_the_start_only() {
    assert!(!SessionId::new("sess_bot_1").is_bot());
}
