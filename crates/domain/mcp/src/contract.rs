//! The MCP capability's contribution points: [`UiSlot`] (artifact themes and
//! UI renderers) and [`AccessRecorderSlot`] (toolset-host access recorders).
//!
//! A member contributes with `c.slot::<UiSlot>()` or
//! `c.slot::<AccessRecorderSlot>()` and the matching builder method; each
//! method stores and runs nothing. Admission drops a disabled member's
//! builder before any factory runs, and the kernel then builds each slot once
//! against the resolved state, in composition order, and inserts the result
//! as `Arc<Built>`. The `mcp` member opens both slots, so the built values
//! are present exactly when that member is admitted.
//!
//! [`UiSlot`] builds a [`UiRendererRegistry`]: core's renderers, then every
//! contributed renderer (a contributed renderer for an artifact type with a
//! built-in replaces it; two contributed renderers for one artifact type are
//! refused with [`SlotError::DuplicateTag`]), and at most one theme (a second
//! is refused with [`SlotError::MultipleEntries`], because two themes have no
//! meaningful winner). [`AccessRecorderSlot`] builds [`AccessRecorders`], every
//! contributed recorder. A factory that fails, or reads a value nothing
//! provides, refuses the build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use systemprompt_extension::{
    Factory, FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder,
};
use systemprompt_identifiers::ExtensionId;

use crate::host::SharedMcpAccessRecorder;
use crate::services::ui_renderer::{ArtifactTheme, UiRenderer, UiRendererRegistry};

const UI_SLOT: &str = "mcp_ui";
const RECORDER_SLOT: &str = "mcp_access_recorders";

type Factories<T> = Vec<Box<dyn Factory<Arc<T>>>>;

/// The artifact UI contribution point, owned by the `mcp` capability.
#[derive(Debug, Clone, Copy)]
pub struct UiSlot;

/// One member's artifact themes and UI renderers, stored until the slot is
/// built.
#[derive(Default)]
pub struct UiSlotBuilder {
    themes: Vec<ArtifactTheme>,
    renderers: Factories<dyn UiRenderer>,
}

impl UiSlotBuilder {
    pub fn theme(&mut self, theme: ArtifactTheme) -> &mut Self {
        self.themes.push(theme);
        self
    }

    pub fn renderer<F: Factory<Arc<dyn UiRenderer>>>(&mut self, f: F) -> &mut Self {
        self.renderers.push(Box::new(f));
        self
    }
}

impl fmt::Debug for UiSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiSlotBuilder")
            .field("themes", &self.themes.len())
            .field("renderers", &self.renderers.len())
            .finish()
    }
}

impl Slot for UiSlot {
    type Builder = UiSlotBuilder;
    type Built = UiRendererRegistry;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut registry = UiRendererRegistry::with_builtins();
        let mut themes: Vec<ArtifactTheme> = Vec::new();
        let mut contributed: HashMap<String, ExtensionId> = HashMap::new();
        for entry in entries {
            themes.extend(entry.builder.themes);
            for renderer in run_all(UI_SLOT, &entry.builder.renderers, &entry.owner, r)? {
                let artifact_type = renderer.artifact_type().to_string();
                if contributed
                    .insert(artifact_type.clone(), entry.owner.clone())
                    .is_some()
                {
                    return Err(SlotError::DuplicateTag {
                        slot: UI_SLOT,
                        tag: artifact_type,
                    });
                }
                if registry.supports(&artifact_type) {
                    tracing::info!(
                        owner = %entry.owner,
                        artifact_type = %artifact_type,
                        "contributed UI renderer replaces the built-in"
                    );
                }
                registry.register_arc(renderer);
            }
        }
        match themes.as_slice() {
            [] => {},
            [theme] => registry.set_theme(*theme),
            _ => {
                return Err(SlotError::MultipleEntries {
                    slot: UI_SLOT,
                    count: themes.len(),
                });
            },
        }
        Ok(registry)
    }
}

/// The toolset-host access-audit contribution point, owned by the `mcp`
/// capability.
#[derive(Debug, Clone, Copy)]
pub struct AccessRecorderSlot;

/// One member's access recorders, stored until the slot is built.
#[derive(Default)]
pub struct AccessRecorderSlotBuilder {
    recorders: Vec<Box<dyn Factory<SharedMcpAccessRecorder>>>,
}

impl AccessRecorderSlotBuilder {
    pub fn recorder<F: Factory<SharedMcpAccessRecorder>>(&mut self, f: F) -> &mut Self {
        self.recorders.push(Box::new(f));
        self
    }
}

impl fmt::Debug for AccessRecorderSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessRecorderSlotBuilder")
            .field("recorders", &self.recorders.len())
            .finish()
    }
}

/// Every contributed access recorder, in composition order.
#[derive(Clone, Default)]
pub struct AccessRecorders(Vec<SharedMcpAccessRecorder>);

impl AccessRecorders {
    #[must_use]
    pub fn to_vec(&self) -> Vec<SharedMcpAccessRecorder> {
        self.0.clone()
    }
}

impl fmt::Debug for AccessRecorders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AccessRecorders")
            .field(&self.0.len())
            .finish()
    }
}

impl Slot for AccessRecorderSlot {
    type Builder = AccessRecorderSlotBuilder;
    type Built = AccessRecorders;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut recorders = Vec::new();
        for entry in entries {
            for factory in &entry.builder.recorders {
                recorders.push(
                    factory
                        .build(r)
                        .map_err(|error| slot_error(RECORDER_SLOT, &entry.owner, error))?,
                );
            }
        }
        Ok(AccessRecorders(recorders))
    }
}

fn run_all<T: ?Sized + 'static>(
    slot: &'static str,
    factories: &Factories<T>,
    owner: &ExtensionId,
    r: &Resolved<'_>,
) -> Result<Vec<Arc<T>>, SlotError> {
    factories
        .iter()
        .map(|factory| {
            factory
                .build(r)
                .map_err(|error| slot_error(slot, owner, error))
        })
        .collect()
}

fn slot_error(slot: &'static str, owner: &ExtensionId, error: FactoryError) -> SlotError {
    match error {
        FactoryError::Missing(missing) => SlotError::Missing(missing),
        FactoryError::Failed(source) => SlotError::Entry {
            slot,
            owner: owner.clone(),
            source,
        },
    }
}
