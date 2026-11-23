use std::sync::Arc;

use systemprompt::reference;
use systemprompt_extension::reserved::{ConferralRow, CoreId, SEALED_CONFERRALS};
use systemprompt_extension::{ProvideError, Resolved, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Conferred, Dependencies};
use systemprompt_runtime::KernelError;

use crate::probe::{Probe, build, build_with, kernel, kernel_with};

#[derive(Clone)]
struct Sealed;

fn conferred(provider: &str, holder: &str, row: &'static str) -> Dependencies {
    let mut state = Dependencies::default();
    state
        .provide_conferred(
            &ExtensionId::new(provider),
            Arc::new(Sealed),
            Conferred {
                holder: ExtensionId::new(holder),
                row,
            },
        )
        .expect("one conferred value");
    state
}

fn route_reads(state: Dependencies, owner: &str, standing: StandingTag) -> bool {
    let kernel = kernel_with(state);
    let owner = ExtensionId::new(owner);
    kernel
        .route_scoped(&owner, standing, &[])
        .get::<Sealed>()
        .is_ok()
}

const ENGINES: &str = ConferralRow::GatewayEngines.name();

#[test]
fn a_listed_conferral_is_read_by_its_core_holder_and_its_provider_only() {
    let listed = || conferred("gateway", "gateway-http", ENGINES);
    assert!(route_reads(listed(), "gateway-http", StandingTag::Core));
    assert!(route_reads(listed(), "gateway", StandingTag::Core));
    for owner in ["files-http", "identity-http", "users", "an-install"] {
        assert!(!route_reads(listed(), owner, StandingTag::Core), "{owner}");
    }
}

#[test]
fn an_install_standing_holder_reads_no_conferral() {
    let listed = conferred("gateway", "gateway-http", ENGINES);
    assert!(!route_reads(listed, "gateway-http", StandingTag::Install));
}

#[test]
fn a_conferral_whose_row_does_not_name_its_provider_holder_or_itself_is_read_by_no_one() {
    let forged_provider = conferred("an-install", "gateway-http", ENGINES);
    assert!(!route_reads(
        forged_provider,
        "gateway-http",
        StandingTag::Core
    ));
    let forged_holder = conferred("gateway", "files-http", ENGINES);
    assert!(!route_reads(forged_holder, "files-http", StandingTag::Core));
    let forged_row = conferred("gateway", "gateway-http", "made-up-row");
    assert!(!route_reads(forged_row, "gateway-http", StandingTag::Core));
}

#[test]
fn a_conferral_is_withheld_from_member_commands_and_runtime_contexts() {
    let kernel = kernel_with(conferred("gateway", "gateway-http", ENGINES));
    assert!(
        Resolved::__member_command(&kernel, kernel.state())
            .optional::<Sealed>()
            .is_none()
    );
    assert!(kernel.runtime_state().find::<Sealed>().is_none());
}

#[test]
fn an_install_providing_under_a_conferral_row_is_refused_before_any_provide_runs() {
    let admitted = build_with(Probe::new("probe-conferral-forger", |c| {
        c.provide_conferred(
            ConferralRow::GatewayEngines,
            |_: &Resolved<'_>| -> Result<Sealed, ProvideError> { Ok(Sealed) },
        );
    }))
    .expect("admission does not judge conferral rows");
    let error = kernel()
        .resolve(&admitted)
        .expect_err("the row names another provider");
    assert!(
        matches!(
            &error,
            KernelError::UnlistedConferral { owner, row }
                if owner.as_str() == "probe-conferral-forger" && *row == ENGINES
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn every_conferral_row_names_the_kernel_or_a_core_provider_of_the_reference_composition_and_another_holder()
 {
    let admitted = build(reference()).expect("the reference composition admits");
    for entry in SEALED_CONFERRALS {
        assert_ne!(entry.provider, entry.holder.id(), "{}", entry.row.name());
        if entry.provider == CoreId::Kernel.id() {
            continue;
        }
        assert_eq!(
            admitted
                .built()
                .standing_of(&ExtensionId::new(entry.provider)),
            Some(StandingTag::Core),
            "{}",
            entry.row.name()
        );
    }
}
