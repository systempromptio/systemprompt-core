use std::sync::Arc;

use systemprompt_brain::BrainExtension;
use systemprompt_extension::DynNotificationSink;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_test_fixtures::{
    lazy_pg_pool, ensure_test_bootstrap, fixture_admitted_of, fixture_brain_kernel,
    fixture_brain_slot,
};

fn offline_pool() -> systemprompt_database::DbPool {
    Arc::new(systemprompt_database::Database::from_pools(lazy_pg_pool(), None))
}

fn kernel(enabled: bool) -> systemprompt_runtime::Kernel {
    ensure_test_bootstrap();
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    fixture_brain_kernel(
        &offline_pool(),
        None,
        notifications,
        BrainProfile {
            enabled,
            ..BrainProfile::default()
        },
    )
}

#[test]
fn no_member_provides_the_brain_handle_cell() {
    let admitted = fixture_admitted_of(vec![Box::new(BrainExtension)]);
    let provided: Vec<&str> = admitted
        .built()
        .members()
        .flat_map(|member| member.sink.provides().iter().map(|entry| entry.type_name()))
        .collect();
    assert!(
        provided
            .iter()
            .all(|name| !name.contains("BrainHandle") && !name.contains("BrainConsumer")),
        "the brain member provides its handle cell: {provided:?}"
    );
}

#[test]
fn the_slot_build_binds_the_handle_it_returns_when_the_brain_is_enabled() {
    let slot = fixture_brain_slot(&kernel(true));
    assert!(slot.is_bound(), "{slot:?}");
}

#[test]
fn a_disabled_brain_leaves_the_returned_handle_unbound_with_its_seams() {
    let slot = fixture_brain_slot(&kernel(false));
    assert!(!slot.is_bound(), "{slot:?}");
    assert!(slot.seams().analyses.is_empty());
}
