use systemprompt_bridge::cli::diagnostics::{GIT_SHA, render, short_sha};

#[test]
fn short_sha_is_at_most_seven_chars() {
    assert!(short_sha().len() <= 7);
}

#[test]
fn short_sha_is_prefix_of_full_sha() {
    assert!(GIT_SHA.starts_with(short_sha()));
    assert_eq!(short_sha().len(), GIT_SHA.len().min(7));
}

#[test]
fn render_contains_expected_markers() {
    let out = render();
    for marker in [
        "systemprompt-bridge",
        "commit:",
        "branch:",
        "built:",
        "os:",
        "paths:",
    ] {
        assert!(out.contains(marker), "render() missing marker: {marker}");
    }
}
