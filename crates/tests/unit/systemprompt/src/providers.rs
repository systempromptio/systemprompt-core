//! A core member's requirement is met only by core members: an install
//! cannot substitute for a disabled core provider (T6).

use systemprompt::CompositionError;
use systemprompt::reference;
use systemprompt_extension::{ProvideError, Resolved};
use systemprompt_traits::content::DynContentCatalogStats;

use crate::probe::{Probe, profile};

fn refuse_provider() -> Result<DynContentCatalogStats, ProvideError> {
    Err(ProvideError::Failed(Box::new(std::fmt::Error)))
}

#[test]
fn install_cannot_substitute_for_a_disabled_core_provider() {
    let error = reference()
        .with(Probe::new("probe-stats", |c| {
            c.provide::<DynContentCatalogStats, _>(|_: &Resolved<'_>| refuse_provider());
        }))
        .build(&profile(&["content"]))
        .expect_err("analytics-http's catalog stats come from core content only");
    assert!(matches!(
        error,
        CompositionError::InstallSubstitution { owner, provider, .. }
            if owner.as_str() == "analytics-http" && provider.as_str() == "probe-stats"
    ));
}

#[test]
fn disabled_core_provider_is_named() {
    let error = reference()
        .build(&profile(&["content"]))
        .expect_err("analytics-http requires the catalog stats content provides");
    assert!(matches!(
        error,
        CompositionError::DisabledDependency { owner, provider, .. }
            if owner.as_str() == "analytics-http" && provider.as_str() == "content"
    ));
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
