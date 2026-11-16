//! A core member's requirement is met only by core members: an install
//! cannot substitute for a disabled core provider (T6). A requirement is
//! also met by the value an enabled member's slot builds, and refused when
//! the only member opening that slot is disabled.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_extension::{ProvideError, Resolved, Slot, SlotEntry, SlotError, SlotOrder};
use systemprompt_traits::content::DynContentCatalogStats;

use crate::probe::{Probe, profile};

struct ProbeSlot;

#[derive(Default)]
struct ProbeSlotBuilder;

struct ProbeSlotBuilt;

impl Slot for ProbeSlot {
    type Builder = ProbeSlotBuilder;
    type Built = ProbeSlotBuilt;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        _entries: Vec<SlotEntry<Self::Builder>>,
        _r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        Ok(ProbeSlotBuilt)
    }
}

fn slot_composition() -> systemprompt::Composition {
    reference()
        .with(Probe::new("probe-slot-owner", |c| {
            c.slot::<ProbeSlot>();
        }))
        .with(Probe::new("probe-slot-reader", |c| {
            c.requires::<Arc<ProbeSlotBuilt>>();
        }))
}

#[test]
fn a_requirement_met_by_a_slot_built_value_is_admitted() {
    slot_composition()
        .build(&profile(&[]))
        .expect("the slot owner's built value meets the reader's requirement");
}

#[test]
fn a_requirement_met_only_by_a_disabled_members_slot_is_refused() {
    let error = slot_composition()
        .build(&profile(&["probe-slot-owner"]))
        .expect_err("the only member opening the slot is disabled");
    assert!(matches!(
        &error,
        CompositionError::DisabledDependency { owner, provider, .. }
            if owner.as_str() == "probe-slot-reader" && provider.as_str() == "probe-slot-owner"
    ));
}

const CONTENT_AND_ITS_EARLIER_DEPENDENTS: [&str; 3] = ["content", "generator", "content-http"];

fn refuse_provider() -> Result<DynContentCatalogStats, ProvideError> {
    Err(ProvideError::Failed(Box::new(std::fmt::Error)))
}

#[test]
fn install_cannot_substitute_for_a_disabled_core_provider() {
    let error = reference()
        .with(Probe::new("probe-stats", |c| {
            c.provide::<DynContentCatalogStats, _>(|_: &Resolved<'_>| refuse_provider());
        }))
        .build(&profile(&CONTENT_AND_ITS_EARLIER_DEPENDENTS))
        .expect_err("analytics-http's catalog stats come from core content only");
    assert!(matches!(
        &error,
        CompositionError::InstallSubstitution { owner, provider, .. }
            if owner.as_str() == "analytics-http" && provider.as_str() == "probe-stats"
    ), "{error:?}");
}

#[test]
fn disabled_core_provider_is_named() {
    let error = reference()
        .build(&profile(&CONTENT_AND_ITS_EARLIER_DEPENDENTS))
        .expect_err("analytics-http requires the catalog stats content provides");
    assert!(matches!(
        &error,
        CompositionError::DisabledDependency { owner, provider, .. }
            if owner.as_str() == "analytics-http" && provider.as_str() == "content"
    ), "{error:?}");
}

#[test]
fn unknown_and_required_disables_are_refused() {
    assert!(matches!(
        reference().build(&profile(&["no-such-member"])),
        Err(CompositionError::UnknownExtension(id)) if id == "no-such-member"
    ));
    assert!(matches!(
        reference().build(&profile(&["users"])),
        Err(CompositionError::RequiredDisabled(id)) if id.as_str() == "users"
    ));
}
