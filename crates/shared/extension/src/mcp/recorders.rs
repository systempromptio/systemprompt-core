//! The toolset-host access-audit contribution point.
//!
//! The `mcp` member opens [`AccessRecorderSlot`]; a member contributes with
//! `c.slot::<AccessRecorderSlot>().recorder(..)`, which stores and runs
//! nothing. The kernel builds every contributed recorder once against the
//! resolved state, in composition order, and inserts [`AccessRecorders`]; a
//! factory that fails, or reads a value nothing provides, refuses the build.
//! The toolset host reads the built value without naming the member that
//! opened the slot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use super::recorder::SharedMcpAccessRecorder;
use crate::{Factory, Resolved, Slot, SlotEntry, SlotError, SlotOrder};

const RECORDER_SLOT: &str = "mcp_access_recorders";

/// The toolset-host access-audit contribution point.
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
                        .map_err(|error| error.into_slot_error(RECORDER_SLOT, &entry.owner))?,
                );
            }
        }
        Ok(AccessRecorders(recorders))
    }
}
