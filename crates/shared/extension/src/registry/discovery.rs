//! Extension discovery that builds the `ExtensionRegistry` from `inventory`
//! submissions and injected extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ExtensionRegistration, ExtensionRegistry};
use crate::error::LoaderError;
use std::sync::Arc;
use tracing::{debug, warn};

impl ExtensionRegistry {
    pub fn discover() -> Result<Self, LoaderError> {
        let mut registry = Self::new();

        debug!("Starting extension discovery via inventory");

        for ext in inventory::iter::<ExtensionRegistration> {
            let ext_arc = (ext.factory)();
            let ext_id = ext_arc.id().to_owned();
            let ext_name = ext_arc.name();
            debug!(
                id = %ext_id,
                name = %ext_name,
                priority = ext_arc.priority(),
                "Discovered extension via inventory"
            );
            registry.extensions.insert(ext_id, Arc::clone(&ext_arc));
            registry.sorted_extensions.push(ext_arc);
        }

        let injected = crate::runtime_config::get_injected_extensions();
        if !injected.is_empty() {
            debug!(
                count = injected.len(),
                "Including injected extensions in discovery"
            );
            for ext in injected {
                let ext_id = ext.id().to_owned();
                if registry.extensions.contains_key(&ext_id) {
                    debug!(
                        id = %ext_id,
                        "Skipping injected extension - already discovered via inventory"
                    );
                    continue;
                }
                debug!(
                    id = %ext_id,
                    name = %ext.name(),
                    priority = ext.priority(),
                    "Including injected extension"
                );
                registry.extensions.insert(ext_id, Arc::clone(&ext));
                registry.sorted_extensions.push(ext);
            }
        }

        registry.sort_by_priority()?;

        let extension_names: Vec<_> = registry
            .sorted_extensions
            .iter()
            .map(|e| e.name())
            .collect();

        if registry.is_empty() {
            warn!(
                "No extensions discovered via inventory. This may indicate LTO stripped the \
                 inventory registrations, or no extensions were registered with \
                 register_extension!(). Check that extension crates are linked and #[used] \
                 attributes are present if using LTO."
            );
        } else {
            debug!(
                count = registry.len(),
                extensions = ?extension_names,
                "Extension discovery completed"
            );
        }

        Ok(registry)
    }

    pub fn discover_and_merge(
        injected: Vec<Arc<dyn crate::Extension>>,
    ) -> Result<Self, LoaderError> {
        let mut registry = Self::discover()?;
        registry.merge(injected)?;
        registry.validate()?;
        Ok(registry)
    }
}
