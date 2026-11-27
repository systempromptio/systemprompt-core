//! One entry of a named registration list of instance-side extensions.
//!
//! The composition names its members: nothing is collected at link time. A
//! list of [`SpExtensionRegistration`]s, each an extension type under its
//! extension id, is built by
//! [`SpExtensions::from_registrations`](crate::extension::SpExtensions::from_registrations).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_identifiers::engine::ExtensionId;

use crate::extension::contract::SpExtension;

/// One registered extension type: its id and the factory that builds it with
/// `Default`.
#[derive(Clone)]
pub struct SpExtensionRegistration {
    pub id: ExtensionId,
    pub factory: fn() -> Arc<dyn SpExtension>,
}

impl SpExtensionRegistration {
    #[must_use]
    pub const fn of<E: SpExtension + Default>(id: ExtensionId) -> Self {
        Self {
            id,
            factory: build::<E>,
        }
    }
}

fn build<E: SpExtension + Default>() -> Arc<dyn SpExtension> {
    Arc::new(E::default())
}

impl fmt::Debug for SpExtensionRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpExtensionRegistration")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
