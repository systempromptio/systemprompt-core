//! Why the instance-side extensions could not be discovered, assembled or
//! applied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_engine::run::extension::ResolveError;
use systemprompt_identifiers::engine::ExtensionId;

use crate::extension::applier::ApplyError;

/// A failure of the instance-side extensions.
#[derive(Debug, thiserror::Error)]
pub enum ExtensionError {
    #[error("the linked extensions do not resolve")]
    Resolve(#[from] ResolveError),
    #[error("extension {extension} hands over an engine half named {engine}")]
    MismatchedHalf {
        extension: ExtensionId,
        engine: ExtensionId,
    },
    #[error("extension {extension} did not take the applied instance")]
    Apply {
        extension: ExtensionId,
        #[source]
        source: ApplyError,
    },
}
