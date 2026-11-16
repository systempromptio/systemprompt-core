use systemprompt::{CORE_MEMBER_IDS, Composition, CompositionError, reference};
use systemprompt_extension::reserved::CoreId;

use crate::probe::{Probe, build, build_with};

fn refused_id(result: Result<systemprompt_extension::Admitted, CompositionError>) -> String {
    match result {
        Err(CompositionError::ReservedId(id)) => id.as_str().to_owned(),
        Err(error) => panic!("expected a reserved-id refusal, got {error:?}"),
        Ok(_) => panic!("expected a reserved-id refusal, the install was admitted"),
    }
}

#[test]
fn an_install_taking_an_unlinked_core_members_id_is_refused() {
    assert_eq!(refused_id(build_with(Probe::new("brain", |_| {}))), "brain");
    assert_eq!(refused_id(build_with(Probe::new("slack", |_| {}))), "slack");
}

#[test]
fn an_install_taking_a_linked_core_members_id_is_refused_even_when_removed() {
    let composition: Composition = reference()
        .without::<systemprompt::scheduler::SchedulerExtension>()
        .with(Probe::new("scheduler", |_| {}));
    assert_eq!(refused_id(build(composition)), "scheduler");
}

#[test]
fn an_install_taking_a_core_id_is_refused() {
    for core in CoreId::ALL {
        assert_eq!(refused_id(build_with(Probe::new(core.id(), |_| {}))), core.id());
    }
}

#[test]
fn an_install_with_a_fresh_id_is_admitted() {
    build_with(Probe::new("probe-fresh", |_| {})).expect("a fresh install id is admitted");
}

#[test]
fn the_list_holds_every_reference_member_and_core_id() {
    for id in reference().member_ids() {
        assert!(CORE_MEMBER_IDS.contains(&id), "{id} is missing from CORE_MEMBER_IDS");
    }
    for core in CoreId::ALL {
        assert!(CORE_MEMBER_IDS.contains(&core.id()), "{core:?} is missing");
    }
}
