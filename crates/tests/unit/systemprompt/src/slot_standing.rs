//! A slot-built value obeys the provider rules: one slot type per built
//! type, a slot value never replaces a seeded or provided value, and an
//! install's slot value never meets a core member's requirement.

use std::sync::Arc;

use systemprompt::gateway::SafetyScannerRegistry;
use systemprompt::governance::contract::{AuthzHookSlot, AuthzHooks};
use systemprompt::{CompositionError, reference};
use systemprompt_extension::contribute::{ContributionSink, build_slot};
use systemprompt_extension::{
    Contributions, Resolved, Slot, SlotEntry, SlotError, SlotOrder, StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;

use crate::probe::{Probe, kernel, profile};

#[derive(Default)]
struct ShadowBuilder;

struct ShadowScanners;

impl Slot for ShadowScanners {
    type Builder = ShadowBuilder;
    type Built = SafetyScannerRegistry;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        _entries: Vec<SlotEntry<Self::Builder>>,
        _r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        Ok(SafetyScannerRegistry::builtin())
    }
}

struct ShadowHooks;

impl Slot for ShadowHooks {
    type Builder = ShadowBuilder;
    type Built = AuthzHooks;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        _entries: Vec<SlotEntry<Self::Builder>>,
        _r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        Ok(AuthzHooks::default())
    }
}

#[test]
fn an_install_slot_building_the_safety_scanner_registry_is_refused() {
    let error = reference()
        .with(Probe::new("probe-shadow-scanners", |c| {
            c.slot::<ShadowScanners>();
        }))
        .build(&profile(&[]))
        .expect_err("the gateway member already builds the scanner registry");
    assert!(
        matches!(
            &error,
            CompositionError::DuplicateProvider { first, second, .. }
                if first.as_str() == "gateway" && second.as_str() == "probe-shadow-scanners"
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn an_install_slot_building_the_authz_hooks_is_refused() {
    let error = reference()
        .with(Probe::new("probe-shadow-hooks", |c| {
            c.slot::<ShadowHooks>();
        }))
        .build(&profile(&[]))
        .expect_err("the authz member already builds the hooks");
    assert!(
        matches!(
            &error,
            CompositionError::DuplicateProvider { first, second, .. }
                if first.as_str() == "authz" && second.as_str() == "probe-shadow-hooks"
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn contributing_to_the_core_slot_is_admitted_as_the_control() {
    reference()
        .with(Probe::new("probe-hook-entry", |c| {
            c.slot::<AuthzHookSlot>();
        }))
        .build(&profile(&[]))
        .expect("an install contributes entries to a core-opened slot");
}

#[test]
fn an_install_slot_value_does_not_meet_a_core_requirement() {
    let disabled = ["content", "content-http", "analytics-http"];
    let error = reference()
        .with(Probe::new("probe-content-slot", |c| {
            c.slot::<systemprompt::content::contract::ContentSlot>();
        }))
        .build(&profile(&disabled))
        .expect_err("a core requirement is never met by an install's slot");
    assert!(
        matches!(
            &error,
            CompositionError::InstallSubstitution { ty, provider, .. }
                if provider.as_str() == "probe-content-slot" && ty.contains("ContentHooks")
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn the_same_core_requirement_names_the_disabled_owner_as_the_control() {
    let disabled = ["content", "content-http", "analytics-http"];
    let error = reference()
        .build(&profile(&disabled))
        .expect_err("the content slot's only opener is disabled");
    assert!(
        matches!(
            &error,
            CompositionError::DisabledDependency { ty, provider, .. }
                if provider.as_str() == "content" && ty.contains("ContentHooks")
        ),
        "unexpected error: {error:?}"
    );
}

fn built_hooks_input(
    standing: StandingTag,
) -> Vec<systemprompt_extension::contribute::SlotBuildInput> {
    let owner = ExtensionId::new("authz");
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(owner.clone(), &mut sink);
    c.slot::<AuthzHookSlot>();
    sink.slots()
        .iter()
        .filter_map(|record| record.take_entry(owner.clone(), standing))
        .collect()
}

#[test]
fn a_slot_value_never_replaces_a_seeded_value() {
    let seeded = Arc::new(AuthzHooks::default());
    let mut state = Dependencies::default().with(Arc::clone(&seeded));
    let built = build_slot(built_hooks_input(StandingTag::Core), &kernel().resolved())
        .expect("the hook slot builds")
        .expect("the member opened the slot");
    let refused = built
        .insert_into(&mut state)
        .expect_err("the seeded hooks are already present");
    assert!(refused.type_name.contains("AuthzHooks"), "{refused:?}");
    let kept = state.get::<Arc<AuthzHooks>>().expect("the seed is kept");
    assert!(Arc::ptr_eq(kept, &seeded));
}

#[test]
fn a_slot_value_enters_an_empty_state_as_the_control() {
    let mut state = Dependencies::default();
    let built = build_slot(built_hooks_input(StandingTag::Core), &kernel().resolved())
        .expect("the hook slot builds")
        .expect("the member opened the slot");
    built
        .insert_into(&mut state)
        .expect("nothing else holds the hooks");
    assert!(state.get::<Arc<AuthzHooks>>().is_ok());
}

#[test]
fn a_slot_entry_built_outside_the_runtime_is_install_standing() {
    let entry = SlotEntry::install(ExtensionId::new("acme"), ShadowBuilder);
    assert_eq!(entry.standing(), StandingTag::Install);
}
