//! The MCP capability's artifact UI contribution point, [`UiSlot`].
//!
//! The slot collects artifact themes and UI renderers. The toolset-host
//! access-recorder slot is part of the extension contract
//! (`systemprompt_extension::mcp::AccessRecorderSlot`); the `mcp` member
//! opens it.
//!
//! A member contributes with `c.slot::<UiSlot>()` and the matching builder
//! method; each
//! method stores and runs nothing. Admission drops a disabled member's
//! builder before any factory runs, and the kernel then builds each slot once
//! against the resolved state, in composition order, and inserts the result
//! as `Arc<Built>`. The `mcp` member opens the slot, so the built value is
//! present exactly when that member is admitted.
//!
//! [`UiSlot`] builds a [`UiRendererRegistry`]: core's renderers, then every
//! contributed renderer (a contributed renderer for an artifact type with a
//! built-in replaces it; two contributed renderers for one artifact type are
//! refused with [`SlotError::DuplicateTag`]), and at most one theme (a second
//! is refused with [`SlotError::MultipleEntries`], because two themes have no
//! meaningful winner). A factory that fails, or reads a value nothing
//! provides, refuses the build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use systemprompt_extension::{
    Factory, Resolved, Slot, SlotEntry, SlotError, SlotOrder, build_factories,
};
use systemprompt_identifiers::ExtensionId;

use crate::services::ui_renderer::{ArtifactTheme, UiRenderer, UiRendererRegistry};

const UI_SLOT: &str = "mcp_ui";

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
            for renderer in build_factories(UI_SLOT, &entry.owner, &entry.builder.renderers, r)? {
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
