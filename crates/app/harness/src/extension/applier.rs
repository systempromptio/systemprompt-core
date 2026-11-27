//! Following the instance applied to the engine: the applied instance as
//! extensions see it, the applier contract, and the set of appliers the sync
//! hands every apply and clear to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use systemprompt_engine::run::extension::{EnabledExtensions, Resolved, admitted};
use systemprompt_device::client::{CredentialSource, SpClient};
use crate::projection::Applied;
use sp_protocol::ids::AccountId;

use crate::extension::enablement::Enablement;
use crate::extension::error::ExtensionError;

/// The instance the engine is governed by.
///
/// It carries the account it belongs to, the manifest as projected, the client
/// that reaches it, the source of its session tokens, the organisation it
/// names and the extensions it enabled for this host (`None` when its manifest
/// names none).
#[derive(Clone)]
pub struct AppliedInstance {
    pub account: AccountId,
    pub applied: Applied,
    pub client: SpClient,
    pub credentials: Arc<dyn CredentialSource>,
    pub organisation: Option<String>,
    pub extensions: Option<EnabledExtensions>,
}

impl fmt::Debug for AppliedInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppliedInstance")
            .field("account", &self.account)
            .field("instance_url", &self.applied.instance_url)
            .field("organisation", &self.organisation)
            .field("extensions", &self.extensions)
            .finish_non_exhaustive()
    }
}

/// Follows the applied instance on behalf of one extension.
///
/// Held as `dyn`. `apply` takes each newly applied instance while the
/// extension is admitted; `clear` runs when the instance goes away or stops
/// admitting the extension, and leaves nothing of the instance behind.
pub trait ManifestApplier: Send + Sync {
    fn apply(&self, instance: &AppliedInstance) -> Result<(), ApplyError>;

    fn clear(&self);
}

/// Why an applier could not take an applied instance.
#[derive(Debug, thiserror::Error)]
#[error("the applied instance could not be taken")]
pub struct ApplyError {
    #[source]
    source: Box<dyn StdError + Send + Sync>,
}

impl ApplyError {
    pub fn new<E: StdError + Send + Sync + 'static>(source: E) -> Self {
        Self {
            source: Box::new(source),
        }
    }
}

/// Every extension's appliers, in resolved order, and the enablement they
/// publish.
pub struct ManifestAppliers {
    entries: Vec<Resolved<Vec<Arc<dyn ManifestApplier>>>>,
    enablement: Enablement,
}

impl ManifestAppliers {
    pub const fn new(
        entries: Vec<Resolved<Vec<Arc<dyn ManifestApplier>>>>,
        enablement: Enablement,
    ) -> Self {
        Self {
            entries,
            enablement,
        }
    }

    pub fn apply(&self, instance: &AppliedInstance) -> Result<(), ExtensionError> {
        let admitted_ids: Vec<_> = admitted(&self.entries, instance.extensions.as_ref())
            .into_iter()
            .map(|entry| &entry.metadata.id)
            .collect();
        let mut first_failure = None;
        for entry in &self.entries {
            let id = &entry.metadata.id;
            if !admitted_ids.contains(&id) {
                entry.value.iter().for_each(|applier| applier.clear());
                continue;
            }
            for applier in &entry.value {
                if let Err(source) = applier.apply(instance) {
                    tracing::warn!(extension = %id, error = %source, "an extension did not take the applied instance");
                    first_failure.get_or_insert(ExtensionError::Apply {
                        extension: id.clone(),
                        source,
                    });
                }
            }
        }
        self.enablement.set(instance.extensions.clone());
        first_failure.map_or(Ok(()), Err)
    }

    pub fn clear(&self) {
        self.enablement.set(None);
        for entry in &self.entries {
            entry.value.iter().for_each(|applier| applier.clear());
        }
    }
}

impl fmt::Debug for ManifestAppliers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManifestAppliers")
            .field(
                "extensions",
                &self
                    .entries
                    .iter()
                    .map(|entry| &entry.metadata.id)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}
