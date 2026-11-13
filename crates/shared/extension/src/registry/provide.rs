//! Builds the [`ExtensionState`] by running every extension's
//! [`Extension::provide`](crate::Extension::provide) in registry order, then
//! checks each declared [`Extension::requires`](crate::Extension::requires)
//! against the result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;

use super::ExtensionRegistry;
use crate::context::ExtensionContext;
use crate::error::LoaderError;
use crate::state::{ExtensionState, StateEntries};

impl ExtensionRegistry {
    pub fn build_state(&self, ctx: &dyn ExtensionContext) -> Result<ExtensionState, LoaderError> {
        let mut entries = StateEntries::default();
        for ext in &self.sorted_extensions {
            let extension = ExtensionId::new(ext.id());
            let mut builder = entries.builder(extension.clone());
            ext.provide(ctx, &mut builder)
                .map_err(|source| LoaderError::ProvideFailed { extension, source })?;
        }
        let state = entries.finish();
        for ext in &self.sorted_extensions {
            if let Some(unmet) = ext
                .requires()
                .into_iter()
                .find(|requirement| !state.contains(requirement))
            {
                return Err(LoaderError::UnmetStateRequirement {
                    extension: ExtensionId::new(ext.id()),
                    type_name: unmet.type_name(),
                });
            }
        }
        Ok(state)
    }
}
