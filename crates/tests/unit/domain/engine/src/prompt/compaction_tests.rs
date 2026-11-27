use systemprompt_engine::prompt::compaction::{SECTIONS, request, summary_message};

#[test]
fn request_contains_every_section_in_order() {
    let template = request(None);
    let mut from = 0;
    for section in SECTIONS {
        let heading = format!("## {section}\n");
        let found = template[from..].find(&heading);
        assert!(found.is_some(), "missing or out of order: {section}");
        from += found.unwrap_or_default() + heading.len();
    }
    for part in ["### Done", "### In progress", "### Blocked"] {
        assert!(template.contains(part));
    }
}

#[test]
fn fresh_request_has_no_previous_summary() {
    let text = request(None);
    assert!(text.contains("## Next steps"));
    assert!(!text.contains("<previous-summary>"));
    assert_eq!(request(Some("   ")), text);
}

#[test]
fn incremental_request_carries_the_previous_summary() {
    let text = request(Some("## Goal\nShip the parser\n"));
    assert!(text.contains("<previous-summary>\n## Goal\nShip the parser\n</previous-summary>"));
    assert!(text.contains("## Relevant files"));
}

#[test]
fn summary_message_wraps_the_summary() {
    let text = summary_message("\n## Goal\nX\n");
    assert!(text.ends_with("## Goal\nX"));
}
