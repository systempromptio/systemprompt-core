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
