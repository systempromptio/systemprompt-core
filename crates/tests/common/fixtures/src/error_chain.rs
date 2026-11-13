//! Renders an error and every `source()` beneath it as one line, so a test
//! asserts on a cause without the cause being repeated in its parent's
//! `Display`.

use std::error::Error;

pub fn error_chain(error: &(dyn Error + 'static)) -> String {
    let mut parts = vec![error.to_string()];
    let mut current = error.source();
    while let Some(cause) = current {
        parts.push(cause.to_string());
        current = cause.source();
    }
    parts.join(": ")
}
