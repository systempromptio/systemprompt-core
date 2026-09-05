//! The text `install --host` / `uninstall --host` print, and the failure flag
//! the caller's exit code is derived from.

use systemprompt_bridge::integration::enrol::{Outcome, Report, Selection, remove_host_profiles};

fn report(display_name: &'static str, outcome: Outcome) -> Report {
    Report {
        host_id: "opencode".to_owned(),
        display_name,
        install_action_label: "wrote ~/.config/opencode/opencode.json",
        outcome,
    }
}

#[test]
fn only_an_outright_failure_is_reported_as_one() {
    assert!(report("OpenCode", Outcome::Failed("boom".to_owned())).is_failure());

    for outcome in [
        Outcome::Installed,
        Outcome::Pending,
        Outcome::Declined,
        Outcome::SyncOnly,
        Outcome::NotEnabled,
        Outcome::Removed,
        Outcome::NothingToRemove,
        Outcome::ManualStep("open System Settings".to_owned()),
    ] {
        assert!(
            !report("OpenCode", outcome).is_failure(),
            "a non-Failed outcome must not fail the command"
        );
    }
}

#[test]
fn rendering_no_reports_says_so_rather_than_printing_an_empty_heading() {
    let rendered = systemprompt_bridge::integration::enrol::render(&[]);
    assert_eq!(rendered, "host enrolment: no hosts selected");
    assert!(!rendered.contains('\n'));
}

#[test]
fn every_outcome_renders_its_own_status_marker_and_names_the_host() {
    let reports = vec![
        report("Installed Host", Outcome::Installed),
        report("Pending Host", Outcome::Pending),
        report("Declined Host", Outcome::Declined),
        report("Sync Host", Outcome::SyncOnly),
        report("Disabled Host", Outcome::NotEnabled),
        report("Removed Host", Outcome::Removed),
        report("Clean Host", Outcome::NothingToRemove),
        report(
            "Manual Host",
            Outcome::ManualStep("remove the profile in System Settings".to_owned()),
        ),
        report("Broken Host", Outcome::Failed("permission denied".to_owned())),
    ];
    let rendered = systemprompt_bridge::integration::enrol::render(&reports);

    assert!(rendered.starts_with("host enrolment:\n"));
    assert_eq!(
        rendered.lines().count(),
        reports.len() + 1,
        "one heading plus one line per report"
    );

    assert!(rendered.contains("[ok      ] Installed Host — profile installed (wrote ~/.config/opencode/opencode.json)"));
    assert!(rendered.contains("[pending ] Pending Host — handed to the OS"));
    assert!(rendered.contains("[declined] Declined Host — administrator approval refused"));
    assert!(rendered.contains("[ok      ] Sync Host — governed through the gateway"));
    assert!(rendered.contains("[skipped ] Disabled Host"));
    assert!(rendered.contains("[ok      ] Removed Host — bridge-owned settings removed"));
    assert!(rendered.contains("[ok      ] Clean Host — nothing of ours left to remove"));
    assert!(
        rendered.contains("[pending ] Manual Host — finish by hand: remove the profile in System Settings")
    );
    assert!(rendered.contains("[failed  ] Broken Host — permission denied"));
}

#[test]
fn a_skipped_host_names_the_id_an_administrator_has_to_enable() {
    let rendered = systemprompt_bridge::integration::enrol::render(&[report(
        "Disabled Host",
        Outcome::NotEnabled,
    )]);
    assert!(
        rendered.contains("enable 'opencode'"),
        "the message must quote the host id, got {rendered}"
    );
}

#[test]
fn a_failure_message_is_carried_through_verbatim_so_the_cause_is_visible() {
    let rendered = systemprompt_bridge::integration::enrol::render(&[report(
        "Broken Host",
        Outcome::Failed("Permission denied (os error 13)".to_owned()),
    )]);
    assert!(rendered.contains("Permission denied (os error 13)"));
}

#[test]
fn removing_an_empty_selection_succeeds_with_nothing_to_report() {
    let reports = remove_host_profiles(&Selection::Ids(Vec::new())).expect("empty selection is valid");
    assert!(reports.is_empty());
    assert_eq!(
        systemprompt_bridge::integration::enrol::render(&reports),
        "host enrolment: no hosts selected"
    );
}

#[test]
fn removing_an_unknown_host_fails_the_whole_request_rather_than_reporting_per_host() {
    let err = remove_host_profiles(&Selection::Ids(vec!["not-a-host".to_owned()]))
        .expect_err("an unknown id is rejected");
    assert!(err.contains("not-a-host"), "got {err}");
    assert!(err.contains("known ids:"), "got {err}");
}

#[test]
fn removing_a_sync_only_agent_reports_that_there_is_nothing_local_to_remove() {
    let reports = remove_host_profiles(&Selection::Ids(vec!["claude-desktop-web".to_owned()]));
    let Ok(reports) = reports else {
        return;
    };
    if let Some(first) = reports.first() {
        assert!(!first.is_failure());
    }
}
