//! Additional error variant tests — covers `ContentError::Service` which is
//! absent from the main error.rs test module.

use systemprompt_content::ContentError;

#[test]
fn content_error_service_display() {
    let err = ContentError::Service("downstream timeout".to_string());
    let msg = err.to_string();
    assert!(msg.contains("service error"));
    assert!(msg.contains("downstream timeout"));
}

#[test]
fn content_error_service_debug() {
    let err = ContentError::Service("dependency unavailable".to_string());
    let debug = format!("{:?}", err);
    assert!(debug.contains("Service"));
}

#[test]
fn content_error_service_matching() {
    let err = ContentError::Service("retry exhausted".to_string());
    match err {
        ContentError::Service(msg) => assert!(msg.contains("retry")),
        _ => panic!("Expected Service variant"),
    }
}

#[test]
fn content_error_service_empty_message() {
    let err = ContentError::Service(String::new());
    assert_eq!(err.to_string(), "service error: ");
}
