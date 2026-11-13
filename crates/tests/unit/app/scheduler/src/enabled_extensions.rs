//! The scheduler runs a contribution only while its owning extension is
//! enabled: disabling an extension in the profile removes its jobs from the
//! runnable set, and leaves every other extension's jobs in place.

use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::ExtensionId;
use systemprompt_scheduler::EnabledExtensions;
use systemprompt_traits::registered_jobs;

fn registry() -> ExtensionRegistry {
    ExtensionRegistry::discover().expect("extension registry")
}

#[test]
fn with_nothing_disabled_every_linked_owner_is_enabled() {
    let enabled = EnabledExtensions::resolve(&registry(), &[]).expect("enabled set");
    assert!(enabled.contains("scheduler"));
    assert!(enabled.job("database_cleanup").is_some());
}

#[test]
fn a_disabled_extension_contributes_no_jobs() {
    let registry = registry();
    let owner = registered_jobs()
        .map(|registration| registration.owner)
        .find(|owner| {
            *owner != "scheduler"
                && registry
                    .enabled_extensions(&[ExtensionId::new(*owner)])
                    .is_ok()
        })
        .expect("a linked, optional extension that owns a job");
    let enabled =
        EnabledExtensions::resolve(&registry, &[ExtensionId::new(owner)]).expect("enabled set");
    assert!(!enabled.contains(owner));
    for registration in registered_jobs().filter(|registration| registration.owner == owner) {
        assert!(
            enabled.job(registration.job.name()).is_none(),
            "{} belongs to the disabled extension {owner}",
            registration.job.name()
        );
    }
    assert!(enabled.job("database_cleanup").is_some());
}

#[test]
fn a_registration_whose_owner_is_not_linked_is_not_run() {
    let enabled = EnabledExtensions::resolve(&registry(), &[]).expect("enabled set");
    assert!(!enabled.contains("no-such-extension"));
}
