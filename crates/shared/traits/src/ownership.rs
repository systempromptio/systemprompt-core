//! The plain-identifier rule for names a declaration hands the kernel to
//! render into SQL.
//!
//! A plain `PostgreSQL` identifier is an ASCII letter or `_`, then ASCII
//! letters, digits or `_`, at most 63 bytes. Admission of member-declared
//! table and column names (owner reassignments) refuses any other spelling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub const fn is_plain_identifier(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }
    if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return false;
    }
    let mut index = 1;
    while index < bytes.len() {
        if !(bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
            return false;
        }
        index += 1;
    }
    true
}
