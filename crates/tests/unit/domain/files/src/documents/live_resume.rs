use systemprompt_files::documents::events::live_topic;
use systemprompt_files::documents::live::ResumeToken;
use systemprompt_identifiers::DocumentId;

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
