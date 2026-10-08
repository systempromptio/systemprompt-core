//! Header names a provider's `extra_headers` may not set.
//!
//! The upstream seam sends the credential, content framing and protocol
//! version itself, and a request builder appends rather than replaces, so an
//! `extra_headers` entry naming one of them would put two values on the wire.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const RESERVED: [&str; 7] = [
    "authorization",
    "x-api-key",
    "x-goog-api-key",
    "anthropic-version",
    "content-type",
    "content-length",
    "host",
];

pub(super) fn reserved_name<'a>(names: impl IntoIterator<Item = &'a String>) -> Option<&'a str> {
    names
        .into_iter()
        .map(String::as_str)
        .find(|name| RESERVED.iter().any(|r| name.eq_ignore_ascii_case(r)))
}
