//! The capability slots the gateway owns.
//!
//! | Slot | Builder method | Built (read with `r.get::<Arc<_>>()`) |
//! |------|----------------|----------------------------------------|
//! | [`RouteSelectorSlot`] | `selector(factory)` | [`RouteSelectorChain`] |
//! | [`SafetyScannerSlot`] | `scanner(name, factory)` | [`SafetyScannerRegistry`] |
//! | [`OverrideSlot`] | `prompt_override(factory)` | [`OverridePolicy`] |
//! | [`OutboundSlot`] | `adapter(tag, factory)` | [`GatewayUpstreamRegistry`] |
//!
//! Every slot is [`SlotOrder::CoreFirst`]: a core member's selector, scanner,
//! override or adapter is consulted before any install's, whatever the
//! composition order. A scanner named like a built-in or like another
//! contributed scanner is refused; an adapter tagged like a built-in
//! (`anthropic`, `openai-chat`, `openai-responses`, `gemini`) is refused with
//! [`SlotError::OutboundShadowsBuiltin`], and two adapters with one tag with
//! [`SlotError::DuplicateTag`]. A gateway built with no entry for a slot uses
//! the slot's default: no selectors, no overrides, the built-in scanners and
//! the built-in adapters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{
    Factory, FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder,
};
use systemprompt_identifiers::ExtensionId;

use crate::policies::{
    OverridePolicy, RouteSelector, RouteSelectorChain, ScannerFactory, SystemPromptOverride,
};
use crate::protocol::OutboundAdapter;
use crate::registry::{GatewayUpstreamRegistry, SafetyScannerRegistry};

const ROUTE_SELECTOR_SLOT: &str = "route_selector";
const SAFETY_SCANNER_SLOT: &str = "safety_scanner";
const OVERRIDE_SLOT: &str = "system_prompt_override";
const OUTBOUND_SLOT: &str = "outbound_adapter";

fn entry_error(slot: &'static str, owner: &ExtensionId, source: FactoryError) -> SlotError {
    match source {
        FactoryError::Missing(missing) => SlotError::Missing(missing),
        FactoryError::Failed(source) => SlotError::Entry {
            slot,
            owner: owner.clone(),
            source,
        },
    }
}

fn build_all<T>(
    slot: &'static str,
    entries: Vec<(ExtensionId, Vec<Box<dyn Factory<T>>>)>,
    r: &Resolved<'_>,
) -> Result<Vec<T>, SlotError> {
    let mut built = Vec::new();
    for (owner, factories) in entries {
        for factory in factories {
            built.push(factory.build(r).map_err(|e| entry_error(slot, &owner, e))?);
        }
    }
    Ok(built)
}

/// Programmatic re-routers consulted after declarative route matching.
#[derive(Debug, Clone, Copy)]
pub struct RouteSelectorSlot;

#[derive(Default)]
pub struct RouteSelectorBuilder {
    selectors: Vec<Box<dyn Factory<Arc<dyn RouteSelector>>>>,
}

impl std::fmt::Debug for RouteSelectorBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteSelectorBuilder")
            .field("selectors", &self.selectors.len())
            .finish()
    }
}

impl RouteSelectorBuilder {
    pub fn selector<F: Factory<Arc<dyn RouteSelector>>>(&mut self, f: F) -> &mut Self {
        self.selectors.push(Box::new(f));
        self
    }
}

impl Slot for RouteSelectorSlot {
    type Builder = RouteSelectorBuilder;
    type Built = RouteSelectorChain;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let entries = entries
            .into_iter()
            .map(|e| (e.owner, e.builder.selectors))
            .collect();
        Ok(RouteSelectorChain::new(build_all(
            ROUTE_SELECTOR_SLOT,
            entries,
            r,
        )?))
    }
}

/// System-prompt overrides consulted after the profile's declarative rules.
#[derive(Debug, Clone, Copy)]
pub struct OverrideSlot;

#[derive(Default)]
pub struct OverrideBuilder {
    overrides: Vec<Box<dyn Factory<Arc<dyn SystemPromptOverride>>>>,
}

impl std::fmt::Debug for OverrideBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverrideBuilder")
            .field("overrides", &self.overrides.len())
            .finish()
    }
}

impl OverrideBuilder {
    pub fn prompt_override<F: Factory<Arc<dyn SystemPromptOverride>>>(
        &mut self,
        f: F,
    ) -> &mut Self {
        self.overrides.push(Box::new(f));
        self
    }
}

impl Slot for OverrideSlot {
    type Builder = OverrideBuilder;
    type Built = OverridePolicy;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let entries = entries
            .into_iter()
            .map(|e| (e.owner, e.builder.overrides))
            .collect();
        Ok(OverridePolicy::new(build_all(OVERRIDE_SLOT, entries, r)?))
    }
}

/// Safety scanners a gateway policy selects by name.
#[derive(Debug, Clone, Copy)]
pub struct SafetyScannerSlot;

#[derive(Default)]
pub struct SafetyScannerBuilder {
    scanners: Vec<(&'static str, Box<dyn Factory<Arc<dyn ScannerFactory>>>)>,
}

impl std::fmt::Debug for SafetyScannerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafetyScannerBuilder")
            .field(
                "scanners",
                &self.scanners.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl SafetyScannerBuilder {
    pub fn scanner<F: Factory<Arc<dyn ScannerFactory>>>(
        &mut self,
        name: &'static str,
        f: F,
    ) -> &mut Self {
        self.scanners.push((name, Box::new(f)));
        self
    }
}

impl Slot for SafetyScannerSlot {
    type Builder = SafetyScannerBuilder;
    type Built = SafetyScannerRegistry;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut registry = SafetyScannerRegistry::builtin();
        for entry in entries {
            for (name, factory) in entry.builder.scanners {
                if registry.contains(name) {
                    return Err(SlotError::DuplicateId {
                        slot: SAFETY_SCANNER_SLOT,
                        id: name.to_owned(),
                    });
                }
                let scanner = factory
                    .build(r)
                    .map_err(|e| entry_error(SAFETY_SCANNER_SLOT, &entry.owner, e))?;
                registry.insert(name, scanner);
            }
        }
        Ok(registry)
    }
}

/// Upstream adapters for tags no built-in adapter serves.
#[derive(Debug, Clone, Copy)]
pub struct OutboundSlot;

#[derive(Default)]
pub struct OutboundBuilder {
    adapters: Vec<(&'static str, Box<dyn Factory<Arc<dyn OutboundAdapter>>>)>,
}

impl std::fmt::Debug for OutboundBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutboundBuilder")
            .field(
                "adapters",
                &self.adapters.iter().map(|(tag, _)| *tag).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl OutboundBuilder {
    pub fn adapter<F: Factory<Arc<dyn OutboundAdapter>>>(
        &mut self,
        tag: &'static str,
        f: F,
    ) -> &mut Self {
        self.adapters.push((tag, Box::new(f)));
        self
    }
}

impl Slot for OutboundSlot {
    type Builder = OutboundBuilder;
    type Built = GatewayUpstreamRegistry;
    const ORDER: SlotOrder = SlotOrder::CoreFirst;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut registry = GatewayUpstreamRegistry::builtin();
        for entry in entries {
            for (tag, factory) in entry.builder.adapters {
                if GatewayUpstreamRegistry::is_builtin(tag) {
                    return Err(SlotError::OutboundShadowsBuiltin {
                        tag: tag.to_owned(),
                        owner: entry.owner,
                    });
                }
                if registry.get(tag).is_some() {
                    return Err(SlotError::DuplicateTag {
                        slot: OUTBOUND_SLOT,
                        tag: tag.to_owned(),
                    });
                }
                let adapter = factory
                    .build(r)
                    .map_err(|e| entry_error(OUTBOUND_SLOT, &entry.owner, e))?;
                registry.insert(tag, adapter);
            }
        }
        Ok(registry)
    }
}
