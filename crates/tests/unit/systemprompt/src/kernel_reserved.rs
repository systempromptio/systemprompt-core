use std::sync::Arc;

use systemprompt::governance::contract::AuthzHookSlot;
use systemprompt::governance::policy::PolicyCatalog;
use systemprompt_extension::contribute::{ContributionSink, SlotBuildInput, build_slot};
use systemprompt_extension::{
    Contributions, MissingDependency, ProvideError, Resolved, StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::KernelError;

use systemprompt::{CompositionError, reference};

use crate::probe::{Probe, build, build_with, kernel};

#[derive(Clone)]
struct FreshValue;

#[test]
fn an_install_providing_a_kernel_inserted_type_is_refused_before_any_provide_runs() {
    let admitted = build_with(Probe::new("probe-policy-catalog", |c| {
        c.provide::<Arc<PolicyCatalog>, _>(
            |_: &Resolved<'_>| -> Result<Arc<PolicyCatalog>, ProvideError> {
                Err(ProvideError::Missing(
                    MissingDependency::of::<PolicyCatalog>(),
                ))
            },
        );
    }))
    .expect("admission does not judge kernel-inserted types");
    let error = kernel()
        .resolve(&admitted)
        .expect_err("the kernel inserts the policy catalog");
    assert!(
        matches!(
            &error,
            KernelError::ReservedType { owner, ty }
                if owner.as_str() == "probe-policy-catalog" && ty.contains("PolicyCatalog")
        ),
        "unexpected error: {error:?}"
    );
}

fn permissive_revocations(
    _: &Resolved<'_>,
) -> Result<systemprompt_models::auth::DynTokenRevocations, ProvideError> {
    Err(ProvideError::Missing(MissingDependency::of::<FreshValue>()))
}

#[test]
fn an_install_cannot_replace_the_oauth_token_revocation_store() {
    let error = build_with(Probe::new("probe-revocations", |c| {
        c.provide(permissive_revocations);
    }))
    .expect_err("the oauth member already provides the store");
    assert!(
        matches!(
            &error,
            CompositionError::DuplicateProvider { first, second, .. }
                if first.as_str() == "oauth" && second.as_str() == "probe-revocations"
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn an_install_cannot_provide_the_token_revocation_store_when_oauth_is_absent() {
    let admitted = build(
        reference()
            .without::<systemprompt::oauth::OauthExtension>()
            .with(Probe::new("probe-revocations-sealed", |c| {
                c.provide_sealed(permissive_revocations);
            })),
    )
    .expect("with no oauth member the sealed provide is not a duplicate");
    let error = kernel()
        .resolve(&admitted)
        .expect_err("only the oauth member provides the store, admitted or not");
    assert!(
        matches!(error, KernelError::CoreOnlyProvide { .. }),
        "unexpected error: {error:?}"
    );
}

#[test]
fn an_install_providing_its_own_type_is_not_refused_as_reserved() {
    let admitted = build_with(Probe::new("probe-fresh-value", |c| {
        c.provide::<FreshValue, _>(|_: &Resolved<'_>| -> Result<FreshValue, ProvideError> {
            Ok(FreshValue)
        });
    }))
    .expect("a fresh provided type is admitted");
    if let Err(error) = kernel().resolve(&admitted) {
        assert!(
            !matches!(error, KernelError::ReservedType { .. }),
            "a fresh type is not reserved: {error:?}"
        );
    }
}

fn hook_inputs(standing: StandingTag) -> Vec<SlotBuildInput> {
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
fn a_core_first_slot_with_only_install_entries_is_absent() {
    let built = build_slot(hook_inputs(StandingTag::Install), &kernel().resolved())
        .expect("an ownerless core-first slot is not an error");
    assert!(
        built.is_none(),
        "install entries alone never build a core-first slot"
    );
}

#[test]
fn a_core_first_slot_with_its_core_owner_is_built() {
    let built = build_slot(hook_inputs(StandingTag::Core), &kernel().resolved())
        .expect("the hook slot builds");
    assert!(built.is_some());
}
