//! The one validation error every identifier returns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// A string that is not a well-formed id of the expected kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{value:?} is not a valid {kind} id")]
pub struct IdError {
    pub kind: &'static str,
    pub value: String,
}
