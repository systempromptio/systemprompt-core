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
//! A [`SlotOrder::CoreFirst`] slot is built only when a core-standing member
//! contributes to it: with its core owner disabled the slot is absent, never
//! built from install entries alone (those entries are logged at `warn`).
//!
//! A built value enters the state through the same duplicate-checked insert
//! a provided value does, owned by the slot's first core-standing
//! contributor (else its first contributor): a slot value never replaces a
//! seeded, provided or other slot-built value. An entry's standing is set by
//! the runtime from the member's admitted standing; a [`SlotEntry`] built
//! outside this crate is always [`StandingTag::Install`].
//!
//! [`Slot::build`] runs with the resolution scoped to that owner's standing,
//! and each entry's factories run scoped to the entry's own owner and
//! standing (`build_factories`, `MemberResolved::within` over
//! [`SlotEntry::scope`]), so an install's
//! factory reads only what an install provide fn may read.
//!
//! The type-erased record ([`SlotRecord`]) is keyed by the slot's `TypeId`
//! and carries a monomorphised build function, so the runtime can build every
//! slot without naming any. It also names the `Arc<S::Built>` type it
//! inserts, so admission can match a member's `requires` against the values
//! slots build without running any member's code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId};
use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Dependencies, DuplicateDependency};

use super::error::SlotError;
use super::member::MemberScope;
use super::resolved::Resolved;
use super::slot_insert::{SlotBuilt, insert_erased};
use super::standing::StandingTag;
use crate::state::StateRequirement;

/// A contribution point owned by a capability; the kernel never names it. A
/// `SEALED` slot's built value enters the state as a sealed provide; a `LATE`
/// slot is built after the after-governance provides.
pub trait Slot: 'static {
    type Builder: Default + Send + 'static;
    type Built: Send + Sync + 'static;
    const ORDER: SlotOrder;
    const SEALED: bool = false;
    const LATE: bool = false;
    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotOrder {
    CoreFirst,
    Composition,
}

#[derive(Debug)]
pub struct SlotEntry<B> {
    pub owner: ExtensionId,
    standing: StandingTag,
    pub builder: B,
}

impl<B> SlotEntry<B> {
    #[must_use]
    pub const fn install(owner: ExtensionId, builder: B) -> Self {
        Self {
            owner,
            standing: StandingTag::Install,
            builder,
        }
    }

    #[must_use]
    pub const fn standing(&self) -> StandingTag {
        self.standing
    }

    #[must_use]
    pub fn scope(&self) -> MemberScope {
        MemberScope::admitted(self.owner.clone(), self.standing)
    }
}

type ErasedBuilder = Box<dyn Any + Send>;
pub(super) type ErasedBuilt = Arc<dyn Any + Send + Sync>;
type BuildFn = fn(Vec<SlotEntry<ErasedBuilder>>, &Resolved<'_>) -> Result<ErasedBuilt, SlotError>;
pub(super) type InsertFn =
    fn(&mut Dependencies, &ExtensionId, ErasedBuilt) -> Result<(), DuplicateDependency>;

/// One member's builder for one slot, with the slot's monomorphised build
/// and insert functions. The builder is taken once, when the slot is built.
#[doc(hidden)]
pub struct SlotRecord {
    slot: TypeId,
    built: StateRequirement,
    order: SlotOrder,
    late: bool,
    builder: Mutex<Option<ErasedBuilder>>,
    build: BuildFn,
    insert: InsertFn,
}

impl SlotRecord {
    pub(crate) fn new<S: Slot>() -> Self {
        Self {
            slot: TypeId::of::<S>(),
            built: StateRequirement::of::<Arc<S::Built>>(),
            order: S::ORDER,
            late: S::LATE,
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

    #[must_use]
    pub const fn late(&self) -> bool {
        self.late
    }

    #[must_use]
    pub const fn built(&self) -> &StateRequirement {
        &self.built
    }

    pub fn take_entry(&self, owner: ExtensionId, standing: StandingTag) -> Option<SlotBuildInput> {
        let builder = self
            .builder
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        Some(SlotBuildInput {
            slot: self.slot,
            built: self.built.type_name(),
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
    built: &'static str,
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

#[doc(hidden)]
pub fn build_slot(
    mut inputs: Vec<SlotBuildInput>,
    r: &Resolved<'_>,
) -> Result<Option<SlotBuilt>, SlotError> {
    let Some(first) = inputs.first() else {
        return Ok(None);
    };
    let (slot, order, build, insert) = (first.slot, first.order, first.build, first.insert);
    let slot_type = first.built;
    let mut owner = first.entry.owner.clone();
    let mut standing = first.entry.standing;
    inputs.retain(|input| input.slot == slot);
    let core_present = inputs
        .iter()
        .any(|input| input.entry.standing == StandingTag::Core);
    if order == SlotOrder::CoreFirst && !core_present {
        let dropped: Vec<&str> = inputs.iter().map(|i| i.entry.owner.as_str()).collect();
        tracing::warn!(
            slot = slot_type,
            owners = ?dropped,
            "core-first slot has no admitted core owner; its install entries are not built"
        );
        return Ok(None);
    }
    if order == SlotOrder::CoreFirst {
        inputs.sort_by_key(|input| match input.entry.standing {
            StandingTag::Core => 0u8,
            StandingTag::Install => 1,
        });
    }
    if let Some(core) = inputs
        .iter()
        .find(|input| input.entry.standing == StandingTag::Core)
    {
        owner = core.entry.owner.clone();
        standing = StandingTag::Core;
    }
    let entries = inputs.into_iter().map(|input| input.entry).collect();
    let value = build(entries, &r.scoped_to(&owner, standing))?;
    Ok(Some(SlotBuilt {
        value,
        insert,
        owner,
    }))
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
