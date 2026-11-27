use harness_ids::JobId;
use harness_prompt::notices::{
    BackgroundJob, MAX_STEPS, POLICY_DENIAL_PREFIX, REMINDER_CLOSE, REMINDER_OPEN,
    background_job_finished, build_switch, invalid_tool_input, plan_mode, steering,
    tool_denied_by_policy, tool_rejected_by_user, unknown_tool,
};

fn is_reminder(text: &str) -> bool {
    text.starts_with(REMINDER_OPEN) && text.ends_with(REMINDER_CLOSE)
}

#[test]
fn reminders_are_wrapped() {
    assert!(is_reminder(&plan_mode(None)));
    assert!(is_reminder(&build_switch()));
    assert!(is_reminder(&steering("also update the docs")));
    assert!(is_reminder(MAX_STEPS));
}

#[test]
fn plan_mode_names_the_plan_file_only_when_given() {
    assert!(!plan_mode(None).contains("exception"));
    assert!(!plan_mode(Some(" ")).contains("exception"));
    assert!(
        plan_mode(Some(r"C:\repo\.harness\plans\next.md"))
            .contains(r"C:\repo\.harness\plans\next.md")
    );
}

#[test]
fn steering_carries_the_message() {
    assert!(steering("  use tabs  ").contains("\n\nuse tabs\n\n"));
}

#[test]
fn background_job_reports_status_and_output() {
    let id = JobId::mint();
    let job = BackgroundJob {
        id: &id,
        command: "npm run dev",
        exit_code: Some(1),
        output: "boom\n",
        output_truncated: true,
    };
    let text = background_job_finished(&job);
    assert!(is_reminder(&text));
    assert!(text.contains(&format!("{id} failed with exit code 1")));
    assert!(text.contains("npm run dev"));
    assert!(text.contains("last part of its output:\n```\nboom\n```"));

    let stopped = BackgroundJob {
        exit_code: None,
        output: "",
        output_truncated: false,
        ..job
    };
    let text = background_job_finished(&stopped);
    assert!(text.contains("was stopped"));
    assert!(text.contains("no output"));
}

#[test]
fn policy_denial_starts_with_the_prefix_and_keeps_the_reason() {
    let text = tool_denied_by_policy("shell", "network access is not allowed");
    assert!(text.starts_with(POLICY_DENIAL_PREFIX));
    assert!(text.contains("network access is not allowed"));
    assert!(tool_denied_by_policy("shell", "  ").contains("no reason was given"));
}

#[test]
fn rejection_and_input_errors_name_the_tool() {
    assert!(tool_rejected_by_user("edit", None).contains("edit call"));
    assert!(tool_rejected_by_user("edit", Some("not that file")).contains("not that file"));
    assert!(invalid_tool_input("read", "missing field path").contains("missing field path"));
    assert!(unknown_tool("bash", &["shell", "read"]).contains("shell, read"));
    assert!(unknown_tool("bash", &[]).contains("text only"));
}
