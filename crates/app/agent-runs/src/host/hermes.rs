//! Hermes: declared, not supported headless.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Execution;

pub(super) const fn execution() -> Execution {
    Execution::Unsupported {
        reason: "Hermes is not installed on any verified host and its headless output has not been \
                 captured, so no decoder exists for it",
    }
}
