use std::error::Error;

use harness_tools::ToolError;

#[test]
fn malformed_keeps_the_cause_and_shows_it() {
    let cause = std::io::Error::other("bad bytes");
    let error = ToolError::malformed("invalid pattern", cause);
    assert_eq!(
        error.to_string(),
        "invalid input: invalid pattern: bad bytes"
    );
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("bad bytes")
    );
}

#[test]
fn malformed_without_context_shows_only_the_cause() {
    let error = ToolError::malformed("", std::io::Error::other("bad bytes"));
    assert_eq!(error.to_string(), "invalid input: bad bytes");
}

#[test]
fn constructors_pick_their_variants() {
    assert_eq!(ToolError::invalid("x").to_string(), "invalid input: x");
    assert_eq!(ToolError::failed("broke").to_string(), "broke");
    let caused = ToolError::caused("could not write", std::io::Error::other("disk full"));
    assert_eq!(caused.to_string(), "could not write");
    assert!(caused.source().is_some());
}
