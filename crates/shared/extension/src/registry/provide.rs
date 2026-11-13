//! Builds the extension state: the host's kernel handles first, then every
//! enabled extension's [`Extension::provide`](crate::Extension::provide) in
//! registry (dependency) order, then each enabled extension's declared
//! [`Extension::requires`](crate::Extension::requires) checked against the
//! result.
//!
//! A disabled extension neither provides nor has its requirements checked.
//! An enabled extension that requires a type only a disabled extension
//! provides fails with [`LoaderError::UnmetStateRequirement`]; one that
//! declares a dependency on a disabled extension fails earlier with
//! [`LoaderError::DisabledDependency`]. No enabled extension silently lacks,
//! or silently receives, state from a disabled one.
//!
//! A requirement is met by any enabled provider, not only a declared
//! dependency: requirements are checked once against the finished state, and
//! every read after boot sees that same finished state, so provider order
//! cannot change the answer. A read *during* `provide` is different — it
//! sees only what was provided before it — which is why an extension that
//! reads another's state while providing must declare it in
//! `dependencies()`; that read fails with a typed
//! [`ExtensionStateError::Missing`](crate::ExtensionStateError::Missing)
//! otherwise, never a stale value.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;

use super::ExtensionRegistry;
use crate::context::ExtensionContext;
use crate::error::LoaderError;
use crate::state::StateBuilder;

impl ExtensionRegistry {
    pub fn build_state(
        &self,
        ctx: &dyn ExtensionContext,
        kernel: Dependencies,
        disabled: &[ExtensionId],
    ) -> Result<Dependencies, LoaderError> {
        let enabled = self.enabled_extensions(disabled)?;
        let mut state = kernel;
        for ext in &enabled {
            let extension = ExtensionId::new(ext.id());
            let mut builder = StateBuilder::new(&mut state, extension.clone());
            ext.provide(ctx, &mut builder)
                .map_err(|source| LoaderError::ProvideFailed { extension, source })?;
        }
        for ext in &enabled {
            if let Some(unmet) = ext
                .requires()
                .into_iter()
                .find(|requirement| !requirement.is_met_by(&state))
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
