//! The errors a contribution, a provide fn, a factory and a slot build can
//! return.
//!
//! A [`ContributionError`] is raised while a member declares: the builder
//! methods only store, so a malformed route path or base is recorded on the
//! value and surfaced once, when the composition reads the member's sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_traits::BoxedSource;

use crate::error::ExtensionConfigError;

#[derive(Debug, thiserror::Error)]
pub enum ContributionError {
    #[error("invalid route path {0}")]
    InvalidPath(&'static str),
    #[error("invalid extension base {0}")]
    InvalidBase(&'static str),
    #[error("route set at {0:?} declared through the wrong builder")]
    RouteSetKind(crate::router::BaseRef),
    #[error("unsupported method {0}")]
    UnsupportedMethod(http::Method),
    #[error("schema {0} declared twice")]
    DuplicateSchema(&'static str),
    #[error("config section invalid")]
    Config(#[from] ExtensionConfigError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProvideError {
    #[error(transparent)]
    Missing(#[from] MissingDependency),
    #[error("provider failed")]
    Failed(#[source] BoxedSource),
}

#[derive(Debug, thiserror::Error)]
pub enum FactoryError {
    #[error(transparent)]
    Missing(#[from] MissingDependency),
    #[error("factory failed")]
    Failed(#[source] BoxedSource),
}

impl FactoryError {
    #[must_use]
    pub fn into_slot_error(self, slot: &'static str, owner: &ExtensionId) -> SlotError {
        match self {
            Self::Missing(missing) => SlotError::Missing(missing),
            Self::Failed(source) => SlotError::Entry {
                slot,
                owner: owner.clone(),
                source,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SlotError {
    #[error("outbound tag {tag} shadows a built-in adapter")]
    OutboundShadowsBuiltin { tag: String, owner: ExtensionId },
    #[error("{slot}: tag {tag} declared twice")]
    DuplicateTag { slot: &'static str, tag: String },
    #[error("{slot}: id {id} declared twice")]
    DuplicateId { slot: &'static str, id: String },
    #[error("{slot} is single-valued; {count} entries")]
    MultipleEntries { slot: &'static str, count: usize },
    #[error("{slot}: {owner}")]
    Entry {
        slot: &'static str,
        owner: ExtensionId,
        #[source]
        source: BoxedSource,
    },
    #[error(transparent)]
    Missing(#[from] MissingDependency),
}
