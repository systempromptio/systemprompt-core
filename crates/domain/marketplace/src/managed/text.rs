//! Browser form transport preserves the original instruction line endings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Result;
use super::error::invalid;

pub fn normalize_form_text(original: &str, submitted: &str) -> Result<String> {
    if original.len() > 1024 * 1024 || submitted.len() > 1024 * 1024 {
        return Err(invalid("Text edits exceed 1 MiB"));
    }
    let without_pairs = original.replace("\r\n", "");
    let crlf = original.contains("\r\n");
    let lf = without_pairs.contains('\n');
    let cr = without_pairs.contains('\r');
    if u8::from(crlf) + u8::from(lf) + u8::from(cr) > 1 {
        return Err(invalid(
            "Mixed line endings require an exact asset revision",
        ));
    }
    let normalized = submitted.replace("\r\n", "\n").replace('\r', "\n");
    Ok(if crlf {
        normalized.replace('\n', "\r\n")
    } else if cr {
        normalized.replace('\n', "\r")
    } else {
        normalized
    })
}
