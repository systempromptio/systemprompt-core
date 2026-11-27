//! Finding every linked instance-side extension and reading its contributions
//! in resolved order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use harness_engine::extension::{
    Contributed, EnabledExtensions, EngineExtension, ExtensionSet, Resolved, admitted, resolve,
};

use crate::applier::ManifestAppliers;
use crate::context::SpContext;
use crate::error::ExtensionError;
use crate::extension::SpExtension;
use crate::register::SpExtensionRegistration;
use crate::service::BackgroundService;

type Entry = Resolved<Arc<dyn SpExtension>>;

/// The resolved instance-side extensions, highest priority first.
///
/// One per id, the highest priority shadowing the rest, dependencies present
/// and acyclic. `provide` runs each extension after the extensions it depends
/// on, so a handle an extension looks up there is already inserted.
#[derive(Clone, Default)]
pub struct SpExtensions {
    entries: Vec<Entry>,
}

impl SpExtensions {
    pub fn discover() -> Result<Self, ExtensionError> {
        let found = inventory::iter::<SpExtensionRegistration>
            .into_iter()
            .map(|registration| (registration.factory)());
        let extensions = Self::from_extensions(found)?;
        tracing::info!(
            count = extensions.entries.len(),
            "instance-side extensions discovered"
        );
        Ok(extensions)
    }

    pub fn from_extensions(
        extensions: impl IntoIterator<Item = Arc<dyn SpExtension>>,
    ) -> Result<Self, ExtensionError> {
        let candidates = extensions.into_iter().map(|value| Resolved {
            metadata: value.metadata(),
            value,
        });
        Ok(Self {
            entries: resolve(candidates)?,
        })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn admitted(&self, enabled: Option<&EnabledExtensions>) -> Vec<&Entry> {
        admitted(&self.entries, enabled)
    }

    pub fn provide(&self, ctx: &SpContext) {
        for entry in self.dependencies_first() {
            entry.value.provide(ctx);
        }
    }

    pub fn engine_set(&self, ctx: &SpContext) -> Result<ExtensionSet, ExtensionError> {
        let mut halves: Vec<Arc<dyn EngineExtension>> = Vec::new();
        for entry in &self.entries {
            let Some(half) = entry.value.engine(ctx) else {
                continue;
            };
            let engine = half.metadata().id;
            if engine != entry.metadata.id {
                return Err(ExtensionError::MismatchedHalf {
                    extension: entry.metadata.id.clone(),
                    engine,
                });
            }
            halves.push(half);
        }
        Ok(ExtensionSet::resolve(halves)?)
    }

    pub fn appliers(&self, ctx: &SpContext) -> ManifestAppliers {
        let entries = self
            .entries
            .iter()
            .map(|entry| Resolved {
                metadata: entry.metadata.clone(),
                value: entry.value.appliers(ctx),
            })
            .collect();
        ManifestAppliers::new(entries, ctx.enablement.clone())
    }

    pub fn services(&self, ctx: &SpContext) -> Vec<Contributed<Arc<dyn BackgroundService>>> {
        self.entries
            .iter()
            .flat_map(|entry| {
                entry
                    .value
                    .services(ctx)
                    .into_iter()
                    .map(|item| Contributed {
                        extension: entry.metadata.id.clone(),
                        item,
                    })
            })
            .collect()
    }
}

impl SpExtensions {
    fn dependencies_first(&self) -> Vec<&Entry> {
        let mut ordered = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            self.place(entry, &mut ordered);
        }
        ordered
    }

    fn place<'a>(&'a self, entry: &'a Entry, ordered: &mut Vec<&'a Entry>) {
        if ordered
            .iter()
            .any(|placed| placed.metadata.id == entry.metadata.id)
        {
            return;
        }
        for dependency in &entry.metadata.dependencies {
            if let Some(target) = self
                .entries
                .iter()
                .find(|candidate| &candidate.metadata.id == dependency)
            {
                self.place(target, ordered);
            }
        }
        ordered.push(entry);
    }
}

impl fmt::Debug for SpExtensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.entries.iter().map(|entry| &entry.metadata))
            .finish()
    }
}
