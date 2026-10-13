use systemprompt_models::a2a::TaskState;

#[test]
fn terminal_states_are_completed_failed_canceled_rejected() {
    assert!(TaskState::Completed.is_terminal());
    assert!(TaskState::Failed.is_terminal());
    assert!(TaskState::Canceled.is_terminal());
    assert!(TaskState::Rejected.is_terminal());
}

#[test]
fn non_terminal_states() {
    assert!(!TaskState::Submitted.is_terminal());
    assert!(!TaskState::Working.is_terminal());
    assert!(!TaskState::InputRequired.is_terminal());
    assert!(!TaskState::AuthRequired.is_terminal());
}

#[test]
fn completed_task_cannot_transition() {
    let all = all_states();
    for target in &all {
        assert!(
            !TaskState::Completed.can_transition_to(target),
            "Completed should not transition to {:?}",
            target
        );
    }
}

#[test]
fn failed_task_cannot_transition() {
    let all = all_states();
    for target in &all {
        assert!(
            !TaskState::Failed.can_transition_to(target),
            "Failed should not transition to {:?}",
            target
        );
    }
}

#[test]
fn canceled_task_cannot_transition() {
    let all = all_states();
    for target in &all {
        assert!(
            !TaskState::Canceled.can_transition_to(target),
            "Canceled should not transition to {:?}",
            target
        );
    }
}

#[test]
fn rejected_task_cannot_transition() {
    let all = all_states();
    for target in &all {
        assert!(
            !TaskState::Rejected.can_transition_to(target),
            "Rejected should not transition to {:?}",
            target
        );
    }
}

#[test]
fn submitted_can_reach_working() {
    assert!(TaskState::Submitted.can_transition_to(&TaskState::Working));
}

#[test]
fn submitted_can_be_canceled() {
    assert!(TaskState::Submitted.can_transition_to(&TaskState::Canceled));
}

#[test]
fn submitted_can_be_rejected() {
    assert!(TaskState::Submitted.can_transition_to(&TaskState::Rejected));
}

#[test]
fn working_can_complete() {
    assert!(TaskState::Working.can_transition_to(&TaskState::Completed));
}

#[test]
fn working_can_fail() {
    assert!(TaskState::Working.can_transition_to(&TaskState::Failed));
}

#[test]
fn working_can_request_input() {
    assert!(TaskState::Working.can_transition_to(&TaskState::InputRequired));
}

#[test]
fn input_required_can_resume_working() {
    assert!(TaskState::InputRequired.can_transition_to(&TaskState::Working));
}

#[test]
fn auth_required_can_resume_working() {
    assert!(TaskState::AuthRequired.can_transition_to(&TaskState::Working));
}

#[test]
fn task_state_serializes_with_prefix() {
    let states = [
        (TaskState::Submitted, "TASK_STATE_SUBMITTED"),
        (TaskState::Working, "TASK_STATE_WORKING"),
        (TaskState::Completed, "TASK_STATE_COMPLETED"),
        (TaskState::Failed, "TASK_STATE_FAILED"),
        (TaskState::Canceled, "TASK_STATE_CANCELED"),
        (TaskState::Rejected, "TASK_STATE_REJECTED"),
        (TaskState::InputRequired, "TASK_STATE_INPUT_REQUIRED"),
        (TaskState::AuthRequired, "TASK_STATE_AUTH_REQUIRED"),
    ];

    for (state, expected) in &states {
        let json = serde_json::to_value(state).unwrap();
        assert_eq!(json.as_str().unwrap(), *expected);
    }
}

fn all_states() -> Vec<TaskState> {
    vec![
        TaskState::Submitted,
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
        TaskState::InputRequired,
        TaskState::AuthRequired,
    ]
}

#[test]
fn removed_v0_3_states_do_not_deserialize() {
    for wire in [
        "TASK_STATE_PENDING",
        "TASK_STATE_UNKNOWN",
        "TASK_STATE_UNSPECIFIED",
    ] {
        assert!(
            serde_json::from_value::<TaskState>(serde_json::json!(wire)).is_err(),
            "{wire} must not deserialize"
        );
    }
}
