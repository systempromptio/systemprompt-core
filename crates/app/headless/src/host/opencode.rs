//! `OpenCode`: declared, not supported headless.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Execution;

pub(super) const fn execution() -> Execution {
    Execution::Unsupported {
        reason: "OpenCode 0.5.1 `run` prints no machine-readable event stream and `serve` is an \
                 HTTP server, so there is no line protocol to supervise",
    }
}
