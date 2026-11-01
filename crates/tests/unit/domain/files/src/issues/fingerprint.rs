use systemprompt_files::issues::fingerprint;
use systemprompt_identifiers::AgentName;
use systemprompt_models::issues::IssueKind;

#[test]
fn whitespace_and_case_do_not_change_the_fingerprint() {
    assert_eq!(
        fingerprint(IssueKind::Bug, None, "Upload   fails\n on PDF"),
        fingerprint(IssueKind::Bug, None, "  upload fails on pdf ")
    );
}

#[test]
fn different_words_give_different_fingerprints() {
    assert_ne!(
        fingerprint(IssueKind::Bug, None, "upload fails"),
        fingerprint(IssueKind::Bug, None, "upload stalls")
    );
}

#[test]
fn the_kind_is_part_of_the_fingerprint() {
    assert_ne!(
        fingerprint(IssueKind::Bug, None, "slow search"),
        fingerprint(IssueKind::Feedback, None, "slow search")
    );
}

#[test]
fn the_filing_agent_is_part_of_the_fingerprint() {
    let direct = fingerprint(IssueKind::Bug, None, "slow search");
    let agent_a = fingerprint(
        IssueKind::Bug,
        Some(&AgentName::new("agent-a")),
        "slow search",
    );
    let agent_b = fingerprint(
        IssueKind::Bug,
        Some(&AgentName::new("agent-b")),
        "slow search",
    );
    assert_ne!(direct, agent_a);
    assert_ne!(agent_a, agent_b);
}

#[test]
fn the_fingerprint_is_lowercase_hex_sha256() {
    let value = fingerprint(
        IssueKind::Feedback,
        Some(&AgentName::new("agent")),
        "anything",
    );
    assert_eq!(value.len(), 64);
    assert!(value.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}
