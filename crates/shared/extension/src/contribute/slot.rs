//! Capability-owned contribution points.
//!
//! A [`Slot`] is defined in its owner crate's `contract` module; the kernel
//! never names one. A member asks for the slot's builder with
//! `Contributions::slot::<S>()` and fills it; every builder method only
//! stores. After admission has dropped every disabled member's builder, the
//! runtime hands the remaining builders to [`Slot::build`] as
//! [`SlotEntry`] values — core members first when the slot's order is
//! [`SlotOrder::CoreFirst`], then composition order — and the built value is
//! inserted into the extension state as `Arc<S::Built>`, which consumers
//! resolve with `Resolved::get::<Arc<S::Built>>()`.
//!
//! The type-erased record ([`SlotRecord`]) is keyed by the slot's `TypeId`
//! and carries a monomorphised build function, so the runtime can build every
//! slot without naming any.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;

use super::error::SlotError;
use super::resolved::Resolved;

/// A contribution point owned by a capability; the kernel never names it.
pub trait Slot: 'static {
    type Builder: Default + Send + 'static;
    type Built: Send + Sync + 'static;
    const ORDER: SlotOrder;
    fn build(entries: Vec<SlotEntry<Self::Builder>>, r: &Resolved<'_>)
    -> Result<Self::Built, SlotError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotOrder {
    CoreFirst,
    Composition,
}

#[derive(Debug)]
pub struct SlotEntry<B> {
    pub owner: ExtensionId,
    pub standing: StandingTag,
    pub builder: B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandingTag {
    Core,
    Install,
}

type ErasedBuilder = Box<dyn Any + Send>;
type ErasedBuilt = Arc<dyn Any + Send + Sync>;
type BuildFn = fn(Vec<SlotEntry<ErasedBuilder>>, &Resolved<'_>) -> Result<ErasedBuilt, SlotError>;
type InsertFn = fn(&mut Dependencies, ErasedBuilt);

/// One member's builder for one slot, with the slot's monomorphised build
/// and insert functions. The builder is taken once, when the slot is built.
#[doc(hidden)]
pub struct SlotRecord {
    slot: TypeId,
    order: SlotOrder,
    builder: Mutex<Option<ErasedBuilder>>,
    build: BuildFn,
    insert: InsertFn,
}

impl SlotRecord {
    pub(crate) fn new<S: Slot>() -> Self {
        Self {
            slot: TypeId::of::<S>(),
            order: S::ORDER,
            builder: Mutex::new(Some(Box::new(S::Builder::default()))),
            build: build_erased::<S>,
            insert: insert_erased::<S>,
        }
    }

    pub(crate) fn builder_mut<S: Slot>(&mut self) -> Option<&mut S::Builder> {
        self.builder
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
            .and_then(|builder| builder.downcast_mut::<S::Builder>())
    }

    #[must_use]
    pub const fn slot(&self) -> TypeId {
        self.slot
    }

    #[must_use]
    pub const fn order(&self) -> SlotOrder {
        self.order
    }

    /// Takes this record's builder for building; a second take is `None`.
    pub fn take_entry(&self, owner: ExtensionId, standing: StandingTag) -> Option<SlotBuildInput> {
        let builder = self
            .builder
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        Some(SlotBuildInput {
            slot: self.slot,
            order: self.order,
            build: self.build,
            insert: self.insert,
            entry: SlotEntry {
                owner,
                standing,
                builder,
            },
        })
    }
}

impl std::fmt::Debug for SlotRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotRecord")
            .field("slot", &self.slot)
            .field("order", &self.order)
            .finish_non_exhaustive()
    }
}

/// One admitted member's entry for one slot, ready to be grouped by slot and
/// built.
#[doc(hidden)]
pub struct SlotBuildInput {
    slot: TypeId,
    order: SlotOrder,
    build: BuildFn,
    insert: InsertFn,
    entry: SlotEntry<ErasedBuilder>,
}

impl SlotBuildInput {
    #[must_use]
    pub const fn slot(&self) -> TypeId {
        self.slot
    }

    #[must_use]
    pub const fn order(&self) -> SlotOrder {
        self.order
    }

    #[must_use]
    pub const fn standing(&self) -> StandingTag {
        self.entry.standing
    }
}

impl std::fmt::Debug for SlotBuildInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotBuildInput")
            .field("slot", &self.slot)
            .field("owner", &self.entry.owner)
            .field("standing", &self.entry.standing)
            .finish_non_exhaustive()
    }
}

/// Builds one slot from every admitted entry for it, ordering core entries
/// first when the slot asks for it, and inserts the built value into `state`.
///
/// Every input must name the same slot; inputs of another slot are ignored.
#[doc(hidden)]
pub fn build_slot(
    mut inputs: Vec<SlotBuildInput>,
    r: &Resolved<'_>,
) -> Result<Option<SlotBuilt>, SlotError> {
    let Some(first) = inputs.first() else {
        return Ok(None);
    };
    let (slot, order, build, insert) = (first.slot, first.order, first.build, first.insert);
    inputs.retain(|input| input.slot == slot);
    if order == SlotOrder::CoreFirst {
        inputs.sort_by_key(|input| match input.entry.standing {
            StandingTag::Core => 0_u8,
            StandingTag::Install => 1,
        });
    }
    let entries = inputs.into_iter().map(|input| input.entry).collect();
    let value = build(entries, r)?;
    Ok(Some(SlotBuilt { value, insert }))
}

/// A built slot value, held until it is inserted into the extension state.
#[doc(hidden)]
pub struct SlotBuilt {
    value: ErasedBuilt,
    insert: InsertFn,
}

impl SlotBuilt {
    pub fn insert_into(self, state: &mut Dependencies) {
        (self.insert)(state, self.value);
    }
}

impl std::fmt::Debug for SlotBuilt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotBuilt").finish_non_exhaustive()
    }
}

fn build_erased<S: Slot>(
    entries: Vec<SlotEntry<ErasedBuilder>>,
    r: &Resolved<'_>,
) -> Result<ErasedBuilt, SlotError> {
    let typed = entries
        .into_iter()
        .filter_map(|entry| {
            entry
                .builder
                .downcast::<S::Builder>()
                .ok()
                .map(|builder| SlotEntry {
                    owner: entry.owner,
                    standing: entry.standing,
                    builder: *builder,
                })
        })
        .collect();
    let built: Arc<S::Built> = Arc::new(S::build(typed, r)?);
    Ok(built)
}

fn insert_erased<S: Slot>(state: &mut Dependencies, value: ErasedBuilt) {
    if let Ok(built) = value.downcast::<S::Built>() {
        state.insert(built);
    }
}
