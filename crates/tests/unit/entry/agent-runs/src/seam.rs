//! The `cli` feature registers the `agent_run_origins` seam with the CLI's
//! listing, owned by the `agent_runs` extension, and the listing marks it
//! `owner_disabled` under a profile that disables `agent_runs`.

use systemprompt_agent_runs_entry as _;
use systemprompt_cli::plugins::seams::{evaluate, linked_seams};
use systemprompt_cli::plugins::types::SeamStatus;
use systemprompt_identifiers::ExtensionId;

const ORIGIN: &str = "sp_test_seam_origin";

systemprompt_agent_runs::register_run_origin!(ORIGIN);

#[test]
fn the_agent_run_origins_seam_lists_the_registered_origins() {
    let seams = linked_seams();
    let origins = seams
        .iter()
        .find(|seam| seam.name == "agent_run_origins")
        .expect("the agent-runs entry crate registers the seam");
    assert_eq!(origins.owner, "systemprompt-agent-runs");
    assert_eq!(origins.extension, Some("agent_runs"));
    assert!(origins.entries.iter().any(|entry| entry == ORIGIN));
}

#[test]
fn the_seam_is_owner_disabled_when_the_profile_disables_agent_runs() {
    let seams: Vec<_> = linked_seams()
        .into_iter()
        .filter(|seam| seam.name == "agent_run_origins")
        .collect();
    let (rows, _) = evaluate(seams, &[], &[ExtensionId::new("agent_runs")]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, SeamStatus::OwnerDisabled);
}
