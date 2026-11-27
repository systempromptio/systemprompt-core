//! Build facts a support screen shows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// What this library is. `core_version` is the systemprompt-core release the
/// binding layer was built from; an app refuses to start on a library whose
/// version is not the one it was generated against.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Diagnostics {
    pub core_version: String,
}

#[must_use]
#[uniffi::export]
pub fn diagnostics() -> Diagnostics {
    Diagnostics {
        core_version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}
