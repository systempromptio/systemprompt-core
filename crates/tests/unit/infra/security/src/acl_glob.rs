//! Tests for the ACL `entity_match` glob primitive.

use systemprompt_security::authz::ingestion::glob::glob_matches;

#[test]
fn star_matches_everything() {
    assert!(glob_matches("*", ""));
    assert!(glob_matches("*", "claude-star-39ccd3"));
    assert!(glob_matches("*", "anything at all"));
}

#[test]
fn exact_match_without_wildcard() {
    assert!(glob_matches("claude-star-39ccd3", "claude-star-39ccd3"));
    assert!(!glob_matches("claude-star-39ccd3", "claude-star-4882a0"));
    assert!(!glob_matches("gemini", "gemini-flash"));
}

#[test]
fn prefix_wildcard() {
    assert!(glob_matches("claude-*", "claude-star-39ccd3"));
    assert!(glob_matches("claude-*", "claude-"));
    assert!(!glob_matches("claude-*", "gpt-4o"));
}

#[test]
fn suffix_wildcard() {
    assert!(glob_matches("*-flash", "gemini-2.5-flash"));
    assert!(!glob_matches("*-flash", "gemini-2.5-pro"));
}

#[test]
fn interior_wildcard() {
    assert!(glob_matches("claude-*-39ccd3", "claude-star-39ccd3"));
    assert!(!glob_matches("claude-*-39ccd3", "claude-star-4882a0"));
}

#[test]
fn overlapping_prefix_and_suffix_does_not_false_match() {
    // The prefix and suffix overlap in the candidate; a naive
    // starts_with/ends_with check would wrongly accept a too-short candidate.
    assert!(!glob_matches("abc*xyz", "abc"));
    assert!(glob_matches("abc*xyz", "abcxyz"));
}

#[test]
fn multibyte_candidate_does_not_panic() {
    assert!(glob_matches("*", "café-☕-server"));
    assert!(glob_matches("café-*", "café-server"));
    assert!(glob_matches("*-☕-*", "a-☕-b"));
    assert!(!glob_matches("café-*", "tea-server"));
}

#[test]
fn candidate_shorter_than_prefix_plus_suffix_never_matches() {
    // Prefix and suffix each match, but the candidate is too short to hold
    // both without them overlapping, so the pattern must still reject it.
    assert!(!glob_matches("a*a", "a"));
    assert!(!glob_matches("abc*abc", "abc"));
    assert!(glob_matches("a*a", "aa"));
}

#[test]
fn adjacent_wildcards_collapse_to_a_single_gap() {
    // An empty interior segment (`**`) is a no-op: it must not force an extra
    // character the way a real interior needle would.
    assert!(glob_matches("a**b", "axb"));
    assert!(glob_matches("a**b", "ab"));
    assert!(glob_matches("pre**suf", "pre-middle-suf"));
}

#[test]
fn interior_needle_absent_rejects() {
    // Prefix and suffix line up but the required interior segment is missing
    // from the gap between them.
    assert!(!glob_matches("a*x*c", "ac"));
    assert!(!glob_matches("a*x*b", "aQb"));
    assert!(glob_matches("a*x*c", "aQxRc"));
}
