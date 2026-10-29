//! A stale-secret rejection names the fix that applies: a Claude Desktop whose
//! machine policy already holds this bridge's credential only needs a restart,
//! one whose policy still holds the credential it presented needs the policy
//! re-applied with administrator approval, and anything else gets the generic
//! re-apply guidance.

use systemprompt_bridge::proxy::dispatch::{StaleSecretRemedy, stale_secret_remedy};

const PRESENTED: &str = "62888ea5";
const OURS: &str = "b6f4c4f5";

#[test]
fn a_policy_holding_this_bridges_credential_means_desktop_needs_a_restart() {
    assert_eq!(
        stale_secret_remedy(true, PRESENTED, OURS, Some(OURS)),
        StaleSecretRemedy::RestartClaudeDesktop
    );
}

#[test]
fn a_policy_holding_the_presented_credential_needs_the_machine_policy_reapplied() {
    assert_eq!(
        stale_secret_remedy(true, PRESENTED, OURS, Some(PRESENTED)),
        StaleSecretRemedy::ReapplyMachinePolicy
    );
}

#[test]
fn no_readable_policy_or_another_client_gets_the_generic_reapply() {
    assert_eq!(
        stale_secret_remedy(true, PRESENTED, OURS, None),
        StaleSecretRemedy::ReapplyProfile
    );
    assert_eq!(
        stale_secret_remedy(true, PRESENTED, OURS, Some("00000000")),
        StaleSecretRemedy::ReapplyProfile
    );
    assert_eq!(
        stale_secret_remedy(false, PRESENTED, OURS, Some(OURS)),
        StaleSecretRemedy::ReapplyProfile,
        "the Desktop policy says nothing about another client's credential"
    );
}
