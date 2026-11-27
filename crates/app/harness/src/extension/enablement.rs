//! The instance's current enablement, shared by everything that shows or runs
//! an extension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_engine::run::extension::EnabledExtensions;
use tokio::sync::watch;

/// The extensions the applied instance enabled, `None` while no instance is
/// applied. [`ManifestAppliers`](crate::extension::ManifestAppliers) sets it on every
/// apply and clears it; screens and services subscribe to follow it.
#[derive(Clone)]
pub struct Enablement {
    sender: Arc<watch::Sender<Option<EnabledExtensions>>>,
}

impl Enablement {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(None);
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn set(&self, enabled: Option<EnabledExtensions>) {
        self.sender.send_replace(enabled);
    }

    pub fn current(&self) -> Option<EnabledExtensions> {
        self.sender.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<EnabledExtensions>> {
        self.sender.subscribe()
    }
}

impl Default for Enablement {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Enablement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Enablement")
            .field(&*self.sender.borrow())
            .finish()
    }
}
