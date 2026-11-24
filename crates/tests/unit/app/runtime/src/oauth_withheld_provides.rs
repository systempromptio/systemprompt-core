//! On a host that seeds no token authority the oauth member withholds its
//! session issuer and plugin-token minter (`ProvideError::Withheld`): both
//! types are reserved and resolve to nothing, so a reader learns the seam is
//! absent instead of holding a value that refuses every call.

use systemprompt_extension::prelude::Dependencies;
use systemprompt_test_fixtures::{fixture_admitted_of, fixture_core_members, offline_resolved_kernel};
use systemprompt_traits::{DynPluginTokenMinter, DynSessionIssuer};

#[test]
fn without_a_token_authority_the_issuer_and_minter_are_withheld() {
    let members = fixture_core_members()
        .into_iter()
        .filter(|member| matches!(member.metadata().id, "users" | "oauth"))
        .collect();
    let kernel = offline_resolved_kernel(&fixture_admitted_of(members), Dependencies::new());
    assert!(kernel.state().find::<DynSessionIssuer>().is_none());
    assert!(kernel.state().find::<DynPluginTokenMinter>().is_none());
}
