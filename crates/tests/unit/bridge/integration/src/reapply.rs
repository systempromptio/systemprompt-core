use systemprompt_bridge::integration::reapply::{Outcome, Report, render};

fn report(display_name: &'static str, outcome: Outcome) -> Report {
    Report {
        display_name,
        install_action_label: "Approve in System Settings",
        outcome,
    }
}

#[test]
fn no_reports_means_every_installed_profile_was_already_current() {
    assert_eq!(
        render(&[]),
        "host profiles: all installed profiles are current",
        "an empty run is a clean bill of health, not silence"
    );
}

#[test]
fn a_reapplied_profile_renders_as_ok() {
    let out = render(&[report("Codex CLI", Outcome::Reapplied)]);
    assert_eq!(
        out,
        "host profiles re-applied:\n  [ok      ] Codex CLI — profile refreshed\n"
    );
}

#[test]
fn a_pending_profile_names_the_action_the_user_still_has_to_take() {
    let out = render(&[report("Claude Desktop", Outcome::Pending)]);
    assert!(
        out.contains("[pending ] Claude Desktop"),
        "a handed-off install is pending, never ok: {out}"
    );
    assert!(
        out.contains("Approve in System Settings"),
        "the install action label is what tells the user how to finish it: {out}"
    );
}

#[test]
fn a_declined_elevation_is_reported_as_a_decision_not_a_failure() {
    let out = render(&[report("Codex CLI", Outcome::Declined)]);
    assert!(out.contains("[declined]"), "{out}");
    assert!(
        out.contains("re-run to retry"),
        "a refused prompt is retryable, and the line says so: {out}"
    );
    assert!(
        !out.contains("[failed  ]"),
        "declining must never be rendered as a fault: {out}"
    );
}

#[test]
fn a_failure_carries_its_own_error_text() {
    let out = render(&[report(
        "Hermes",
        Outcome::Failed("loopback secret: no key on disk".to_owned()),
    )]);
    assert!(
        out.contains("[failed  ] Hermes — loopback secret: no key on disk"),
        "the cause must survive into the rendered line: {out}"
    );
}

#[test]
fn every_report_gets_its_own_line_under_one_header() {
    let out = render(&[
        report("Codex CLI", Outcome::Reapplied),
        report("Hermes", Outcome::Failed("boom".to_owned())),
        report("OpenCode", Outcome::Declined),
    ]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 4, "one header plus one line per report: {out}");
    assert_eq!(lines[0], "host profiles re-applied:");
    assert!(lines[1].contains("Codex CLI"), "{out}");
    assert!(lines[2].contains("Hermes"), "{out}");
    assert!(lines[3].contains("OpenCode"), "{out}");
}
