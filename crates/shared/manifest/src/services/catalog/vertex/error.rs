//! Failure modes of reading the embedded Vertex rate card.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ModelId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VertexRateCardError {
    #[error("embedded Vertex rate card failed to parse: {0}")]
    Parse(#[source] serde_yaml::Error),

    #[error("embedded Vertex rate card entry '{entry}' is invalid: {defect}")]
    InvalidEntry {
        entry: ModelId,
        defect: VertexRateCardDefect,
    },
}

/// The rule an embedded Vertex rate card entry breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum VertexRateCardDefect {
    #[error("`docs` must be the official documentation URL")]
    DocsNotOfficial,

    #[error("`retires_on` is not after `released`")]
    RetiresBeforeRelease,

    #[error("`price_until` is not after `released`")]
    PriceUntilBeforeRelease,
}
