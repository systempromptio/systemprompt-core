//! [`EngineError`]: what an engine reports, with its own error as source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

type Source = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("the update is not a valid update of this engine")]
    InvalidUpdate(#[source] Source),
    #[error("the version is not a valid version of this engine")]
    InvalidVersion(#[source] Source),
    #[error("the engine could not update the document state")]
    State(#[source] Source),
    #[error("the engine could not encode the document state")]
    Encode(#[source] Source),
    #[error("the engine holds content that does not read as a model")]
    Model(#[source] Source),
    #[error(
        "the update carries operations of engine peer {peer}, which the caller was not assigned"
    )]
    PeerNotAssigned { peer: u64 },
    #[error("the engine does not support this model")]
    Unsupported,
}
