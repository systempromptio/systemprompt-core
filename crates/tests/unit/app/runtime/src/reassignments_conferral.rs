//! The kernel records the admitted owner reassignments sealed under the
//! `identity-owner-reassignments` conferral row, so only a command
//! `identity-http` owns with core standing reads them.

use std::sync::Arc;

use systemprompt_extension::reserved::{ConferralRow, CoreId};
use systemprompt_extension::{AdmittedOwnerReassignments, Resolved, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::Kernel;
use systemprompt_test_fixtures::{ComposedParts, probe_admitted};
use systemprompt_traits::BackgroundTasks;

type Reassignments = Arc<AdmittedOwnerReassignments>;

fn resolved_kernel() -> Kernel {
    let mut kernel = Kernel::detached(BackgroundTasks::new(), Dependencies::default());
    kernel
        .resolve(&probe_admitted(ComposedParts::default()))
        .expect("an empty composition resolves");
    kernel
}

#[test]
fn the_kernel_confers_the_owner_reassignments_on_identity_http_alone() {
    let kernel = resolved_kernel();
    let state = kernel.state();
    assert!(state.is_sealed::<Reassignments>());
    let conferred = state
        .conferral_of::<Reassignments>()
        .expect("the reassignments are conferred");
    assert_eq!(conferred.holder.as_str(), CoreId::IdentityHttp.id());
    assert_eq!(
        conferred.row,
        ConferralRow::IdentityOwnerReassignments.name()
    );
}

#[test]
fn only_an_identity_http_command_with_core_standing_reads_the_reassignments() {
    let kernel = resolved_kernel();
    let state = kernel.state();
    let reads = |owner: &str, standing| {
        let owner = ExtensionId::new(owner);
        Resolved::__member_command_owned(&kernel, state, &owner, standing)
            .optional::<Reassignments>()
            .is_some()
    };
    assert!(reads("identity-http", StandingTag::Core));
    assert!(!reads("identity-http", StandingTag::Install));
    assert!(!reads("an-install", StandingTag::Install));
    assert!(!reads("gateway-http", StandingTag::Core));
    assert!(
        Resolved::__member_command(&kernel, state)
            .optional::<Reassignments>()
            .is_none()
    );
    assert!(kernel.runtime_state().find::<Reassignments>().is_none());
}
