//! The `x-systemprompt-memory` request header: only `off`, in any case,
//! turns team-memory recall off for the request.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::routes::gateway::messages::extract::memory_preference;
use systemprompt_identifiers::headers::MEMORY;
use systemprompt_models::origin::MemoryPreference;

fn with_memory(value: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(MEMORY, HeaderValue::from_static(value));
    headers
}

#[test]
fn lowercase_off_disables_recall() {
    assert_eq!(
        memory_preference(&with_memory("off")),
        MemoryPreference::Off
    );
}

#[test]
fn uppercase_off_disables_recall() {
    assert_eq!(
        memory_preference(&with_memory("OFF")),
        MemoryPreference::Off
    );
}

#[test]
fn any_other_value_keeps_the_default() {
    assert_eq!(
        memory_preference(&with_memory("on")),
        MemoryPreference::Default
    );
}

#[test]
fn an_absent_header_keeps_the_default() {
    assert_eq!(
        memory_preference(&HeaderMap::new()),
        MemoryPreference::Default
    );
}
