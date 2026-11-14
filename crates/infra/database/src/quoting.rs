//! The one `PostgreSQL` identifier quoter.
//!
//! [`quote_identifier`] wraps any identifier in double quotes and doubles
//! every embedded double quote, so the result is a single quoted identifier
//! whatever the input holds. It quotes; it does not validate. A name that
//! must also pass the identifier allowlist goes through `SafeIdentifier`
//! instead, which rejects names such as `uuid-ossp` that are legal once
//! quoted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn quote_identifier(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}
