//! `TaskState::from_str` is the one parser: it accepts the A2A wire names and
//! the short names the HTTP filter and task-notification paths use, and
//! rejects everything else, including the British spelling of canceled.

use systemprompt_models::a2a::TaskState;

#[test]
fn short_names_parse_to_the_same_states_as_the_wire_names() {
    let pairs = [
        ("submitted", "TASK_STATE_SUBMITTED", TaskState::Submitted),
        ("working", "TASK_STATE_WORKING", TaskState::Working),
        ("completed", "TASK_STATE_COMPLETED", TaskState::Completed),
        ("failed", "TASK_STATE_FAILED", TaskState::Failed),
        ("canceled", "TASK_STATE_CANCELED", TaskState::Canceled),
        ("rejected", "TASK_STATE_REJECTED", TaskState::Rejected),
        (
            "input-required",
            "TASK_STATE_INPUT_REQUIRED",
            TaskState::InputRequired,
        ),
        (
            "auth-required",
            "TASK_STATE_AUTH_REQUIRED",
            TaskState::AuthRequired,
        ),
    ];
    for (short, wire, expected) in pairs {
        assert_eq!(short.parse::<TaskState>().expect(short), expected);
        assert_eq!(wire.parse::<TaskState>().expect(wire), expected);
        assert_eq!(expected.as_str(), wire);
    }
}

#[test]
fn unknown_and_retired_spellings_are_rejected() {
    for value in [
        "cancelled",
        "CANCELED",
        "Working",
        "input_required",
        "",
        "done",
    ] {
        let err = value
            .parse::<TaskState>()
            .expect_err(&format!("{value:?} must not parse"));
        assert!(err.to_string().contains("task state"), "{err}");
    }
}
