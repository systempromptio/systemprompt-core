//! A re-apply that cannot be completed by us must not read as success.
//!
//! macOS Claude Desktop installs its profile with `open -g <mobileconfig>`,
//! which hands the file to System Settings and returns `Ok(())` whether or not
//! the user ever approves it. Reporting that as "refreshed" is how a profile
//! stays stale while every tool insists it was fixed, so the outcome is decided
//! by re-probing the host — and the rendering has to keep the two apart.

use systemprompt_bridge::integration::reapply::{Outcome, Report, render};

const fn report(display_name: &'static str, outcome: Outcome) -> Report {
    Report {
        display_name,
        install_action_label: "loaded into managed preferences",
        outcome,
    }
}

#[test]
fn nothing_stale_is_stated_rather_than_left_blank() {
    let out = render(&[]);
    assert!(out.contains("all installed profiles are current"), "{out}");
}

#[test]
fn pending_is_not_reported_as_refreshed() {
    let out = render(&[report("Claude Desktop", Outcome::Pending)]);
    assert!(out.contains("[pending"), "{out}");
    assert!(out.contains("approve it to finish"), "{out}");
    assert!(
        !out.contains("profile refreshed"),
        "a profile the OS has not accepted must not read as refreshed: {out}"
    );
}

#[test]
fn the_four_outcomes_are_distinguishable() {
    let out = render(&[
        report("A", Outcome::Reapplied),
        report("B", Outcome::Pending),
        report("C", Outcome::Declined),
        report("D", Outcome::Failed("boom".to_owned())),
    ]);
    assert!(out.contains("[ok"), "{out}");
    assert!(out.contains("[pending"), "{out}");
    // Declining the administrator prompt is a decision, not a fault.
    assert!(out.contains("[declined"), "{out}");
    assert!(out.contains("[failed"), "{out}");
    assert!(out.contains("boom"), "{out}");
}
