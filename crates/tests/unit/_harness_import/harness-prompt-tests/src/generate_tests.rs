use harness_prompt::generate::{TITLE_MAX_CHARS, clean_title, summary_request, title_request};

#[test]
fn title_request_fences_the_message() {
    let text = title_request("  how do I run the tests?  ");
    assert!(text.ends_with("<message>\nhow do I run the tests?\n</message>"));
}

#[test]
fn clean_title_strips_decoration() {
    assert_eq!(
        clean_title("\"Fix login redirect loop\"").as_deref(),
        Some("Fix login redirect loop")
    );
    assert_eq!(
        clean_title("Title: Explain the cache layer.").as_deref(),
        Some("Explain the cache layer")
    );
    assert_eq!(
        clean_title("**Title:** `Parser rewrite`").as_deref(),
        Some("Parser rewrite")
    );
    assert_eq!(
        clean_title("\n\n  Add   retry\tlogic \nsecond line").as_deref(),
        Some("Add retry logic")
    );
    assert_eq!(
        clean_title("<think>hmm</think>\nRename config keys").as_deref(),
        Some("Rename config keys")
    );
    assert_eq!(
        clean_title("„Datenbank migrieren“").as_deref(),
        Some("Datenbank migrieren")
    );
}

#[test]
fn clean_title_rejects_empty_output() {
    assert_eq!(clean_title(""), None);
    assert_eq!(clean_title("  \n\"\"  "), None);
}

#[test]
fn clean_title_caps_length_on_char_boundaries() {
    let long = "é".repeat(TITLE_MAX_CHARS + 20);
    let title = clean_title(&long);
    assert_eq!(
        title.map(|title| title.chars().count()),
        Some(TITLE_MAX_CHARS)
    );
}

#[test]
fn summary_request_handles_no_changes() {
    assert!(summary_request("  ").contains("changed no files"));
    assert!(summary_request("src\\main.rs +3 -1").ends_with("src\\main.rs +3 -1"));
}
