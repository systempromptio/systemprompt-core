//! Unit tests for slug generation utilities
//!
//! Tests cover:
//! - Basic slug generation from names
//! - Special character handling
//! - Whitespace normalization
//! - Unique slug generation with collision handling

use systemprompt_agent::services::shared::slug::{generate_slug, generate_unique_slug};

// ============================================================================
// Basic Slug Generation Tests
// ============================================================================

#[test]
fn test_generate_slug_simple_name() {
    let slug = generate_slug("Test Agent");
    assert_eq!(slug, "test-agent");
}

#[test]
fn test_generate_slug_already_lowercase() {
    let slug = generate_slug("my-agent");
    assert_eq!(slug, "my-agent");
}

#[test]
fn test_generate_slug_uppercase_input() {
    let slug = generate_slug("MY AGENT NAME");
    assert_eq!(slug, "my-agent-name");
}

#[test]
fn test_generate_slug_mixed_case() {
    let slug = generate_slug("MyAgentName");
    assert_eq!(slug, "myagentname");
}

#[test]
fn test_generate_slug_with_numbers() {
    let slug = generate_slug("Agent 123");
    assert_eq!(slug, "agent-123");
}

#[test]
fn test_generate_slug_numbers_only() {
    let slug = generate_slug("12345");
    assert_eq!(slug, "12345");
}

// ============================================================================
// Special Character Handling Tests
// ============================================================================

#[test]
fn test_generate_slug_with_underscores() {
    let slug = generate_slug("my_agent_name");
    assert_eq!(slug, "my-agent-name");
}

#[test]
fn test_generate_slug_with_dots() {
    let slug = generate_slug("agent.v1.0");
    assert_eq!(slug, "agent-v1-0");
}

#[test]
fn test_generate_slug_with_special_characters() {
    let slug = generate_slug("Agent@#$%Special");
    assert_eq!(slug, "agentspecial");
}

#[test]
fn test_generate_slug_with_parentheses() {
    let slug = generate_slug("Agent (Beta)");
    assert_eq!(slug, "agent-beta");
}

#[test]
fn test_generate_slug_with_ampersand() {
    let slug = generate_slug("Search & Rescue");
    assert_eq!(slug, "search-rescue");
}

#[test]
fn test_generate_slug_with_unicode() {
    let slug = generate_slug("Café Agent");
    // Unicode characters like 'é' are kept as alphanumeric
    assert_eq!(slug, "café-agent");
}

// ============================================================================
// Whitespace Normalization Tests
// ============================================================================

#[test]
fn test_generate_slug_multiple_spaces() {
    let slug = generate_slug("Agent    Multiple    Spaces");
    assert_eq!(slug, "agent-multiple-spaces");
}

#[test]
fn test_generate_slug_leading_trailing_spaces() {
    let slug = generate_slug("  Agent Name  ");
    assert_eq!(slug, "agent-name");
}

#[test]
fn test_generate_slug_tabs_and_newlines() {
    let slug = generate_slug("Agent\tWith\nTabs");
    assert_eq!(slug, "agent-with-tabs");
}

#[test]
fn test_generate_slug_empty_string() {
    let slug = generate_slug("");
    assert_eq!(slug, "");
}

#[test]
fn test_generate_slug_only_spaces() {
    let slug = generate_slug("   ");
    assert_eq!(slug, "");
}

#[test]
fn test_generate_slug_only_special_chars() {
    let slug = generate_slug("@#$%^&*");
    assert_eq!(slug, "");
}

// ============================================================================
// Consecutive Hyphen Handling Tests
// ============================================================================

#[test]
fn test_generate_slug_consecutive_hyphens() {
    let slug = generate_slug("Agent---Name");
    assert_eq!(slug, "agent-name");
}

#[test]
fn test_generate_slug_mixed_separators() {
    let slug = generate_slug("Agent - _ . Name");
    assert_eq!(slug, "agent-name");
}

#[test]
fn test_generate_slug_leading_hyphens() {
    let slug = generate_slug("---Agent");
    assert_eq!(slug, "agent");
}

#[test]
fn test_generate_slug_trailing_hyphens() {
    let slug = generate_slug("Agent---");
    assert_eq!(slug, "agent");
}

// ============================================================================
// Unique Slug Generation Tests
// ============================================================================

#[test]
fn test_generate_unique_slug_no_collision() {
    let existing: Vec<String> = vec![];
    let slug = generate_unique_slug("Test Agent", &existing);
    assert_eq!(slug, "test-agent");
}

#[test]
fn test_generate_unique_slug_with_collision() {
    let existing = vec!["test-agent".to_string()];
    let slug = generate_unique_slug("Test Agent", &existing);
    assert_eq!(slug, "test-agent-1");
}

#[test]
fn test_generate_unique_slug_multiple_collisions() {
    let existing = vec![
        "test-agent".to_string(),
        "test-agent-1".to_string(),
        "test-agent-2".to_string(),
    ];
    let slug = generate_unique_slug("Test Agent", &existing);
    assert_eq!(slug, "test-agent-3");
}

#[test]
fn test_generate_unique_slug_gap_in_sequence() {
    let existing = vec![
        "test-agent".to_string(),
        "test-agent-1".to_string(),
        "test-agent-3".to_string(),
    ];
    let slug = generate_unique_slug("Test Agent", &existing);
    assert_eq!(slug, "test-agent-2");
}

#[test]
fn test_generate_unique_slug_different_base() {
    let existing = vec!["other-agent".to_string(), "another-agent".to_string()];
    let slug = generate_unique_slug("Test Agent", &existing);
    assert_eq!(slug, "test-agent");
}

#[test]
fn test_generate_unique_slug_empty_name() {
    let existing = vec!["".to_string()];
    let slug = generate_unique_slug("", &existing);
    assert_eq!(slug, "-1");
}

#[test]
fn test_generate_unique_slug_preserves_numbers() {
    let existing: Vec<String> = vec![];
    let slug = generate_unique_slug("Agent v2.0", &existing);
    assert_eq!(slug, "agent-v2-0");
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[test]
fn test_generate_slug_single_character() {
    let slug = generate_slug("A");
    assert_eq!(slug, "a");
}

#[test]
fn test_generate_slug_single_number() {
    let slug = generate_slug("1");
    assert_eq!(slug, "1");
}

#[test]
fn test_generate_slug_very_long_name() {
    let long_name = "A".repeat(1000);
    let slug = generate_slug(&long_name);
    assert_eq!(slug.len(), 1000);
    assert!(slug.chars().all(|c| c == 'a'));
}

#[test]
fn test_generate_slug_alphanumeric_mix() {
    let slug = generate_slug("Agent2024Test");
    assert_eq!(slug, "agent2024test");
}

#[test]
fn test_generate_slug_hyphen_in_middle() {
    let slug = generate_slug("pre-existing-name");
    assert_eq!(slug, "pre-existing-name");
}
