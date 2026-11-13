//! Renders an error and every `source()` beneath it as one line, so a test
//! asserts on a cause without the cause being repeated in its parent's
//! `Display`.

use std::error::Error;

pub fn error_chain(error: &(dyn Error + 'static)) -> String {
    std::iter::successors(Some(error), |current| current.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}
