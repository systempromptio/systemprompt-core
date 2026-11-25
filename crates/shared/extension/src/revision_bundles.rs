//! The managed revision bundle contract: a published revision closure, loaded
//! on behalf of its owner by a capability outside the marketplace.
//!
//! The `managed_resources` member owns the revisions and provides them as
//! [`DynRevisionBundleSource`] after governance, open as its managed
//! repository is. The agent-run runner installs a run's harness from it; a
//! composition without that member provides none, and a run that names a
//! harness then finishes failed with the source named as absent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{ResourceRevisionId, UserId};
use systemprompt_models::managed::RevisionBundle;
use systemprompt_traits::BoxedSource;

#[derive(Debug, thiserror::Error)]
#[error("the managed revision bundle could not be loaded: {0}")]
pub struct RevisionBundleLoadError(#[source] pub BoxedSource);

/// Loads a revision closure on behalf of its owner.
///
/// `#[async_trait]` because the source is held as `Arc<dyn
/// RevisionBundleSource>`.
#[async_trait]
pub trait RevisionBundleSource: Send + Sync {
    async fn revision_bundle(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<RevisionBundle, RevisionBundleLoadError>;
}

pub type DynRevisionBundleSource = Arc<dyn RevisionBundleSource>;
