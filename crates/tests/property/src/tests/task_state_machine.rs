use proptest::prelude::*;
use systemprompt_models::a2a::TaskState;

use crate::strategies::a2a::arb_task_state;

fn all_states() -> Vec<TaskState> {
    vec![
        TaskState::Pending,
        TaskState::Submitted,
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
        TaskState::InputRequired,
        TaskState::AuthRequired,
        TaskState::Unknown,
    ]
}

proptest! {
    #[test]
    fn terminal_states_block_all_transitions(target in arb_task_state()) {
        let terminal_states = [
            TaskState::Completed,
            TaskState::Failed,
            TaskState::Canceled,
            TaskState::Rejected,
        ];

        for terminal in &terminal_states {
            prop_assert!(
                !terminal.can_transition_to(&target),
                "Terminal state {:?} should not transition to {:?}",
                terminal,
                target
            );
        }
    }

    #[test]
    fn terminal_states_are_terminal(state in arb_task_state()) {
        let expected = matches!(
            state,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled | TaskState::Rejected
        );
        prop_assert_eq!(state.is_terminal(), expected);
    }

    #[test]
    fn random_transition_sequence_never_leaves_terminal(
        initial in arb_task_state(),
        transitions in proptest::collection::vec(arb_task_state(), 1..20)
    ) {
        let mut current = initial;
        let mut reached_terminal = false;

        for target in &transitions {
            if reached_terminal {
                prop_assert!(
                    !current.can_transition_to(target),
                    "After reaching terminal state {:?}, transition to {:?} should be blocked",
                    current,
                    target
                );
            } else if current.can_transition_to(target) {
                current = *target;
                if current.is_terminal() {
                    reached_terminal = true;
                }
            }
        }
    }
}

#[test]
fn pending_can_only_transition_to_submitted() {
    let pending = TaskState::Pending;
    for state in all_states() {
        let expected = matches!(state, TaskState::Submitted);
        assert_eq!(
            pending.can_transition_to(&state),
            expected,
            "Pending -> {:?} should be {}",
            state,
            expected
        );
    }
}

#[test]
fn submitted_valid_transitions() {
    let submitted = TaskState::Submitted;
    let valid_targets = [
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Rejected,
        TaskState::AuthRequired,
    ];

    for state in all_states() {
        let expected = valid_targets.contains(&state);
        assert_eq!(
            submitted.can_transition_to(&state),
            expected,
            "Submitted -> {:?} should be {}",
            state,
            expected
        );
    }
}

#[test]
fn working_valid_transitions() {
    let working = TaskState::Working;
    let valid_targets = [
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::InputRequired,
    ];

    for state in all_states() {
        let expected = valid_targets.contains(&state);
        assert_eq!(
            working.can_transition_to(&state),
            expected,
            "Working -> {:?} should be {}",
            state,
            expected
        );
    }
}

#[test]
fn input_required_valid_transitions() {
    let input_required = TaskState::InputRequired;
    let valid_targets = [
        TaskState::Working,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
    ];

    for state in all_states() {
        let expected = valid_targets.contains(&state);
        assert_eq!(
            input_required.can_transition_to(&state),
            expected,
            "InputRequired -> {:?} should be {}",
            state,
            expected
        );
    }
}

#[test]
fn auth_required_valid_transitions() {
    let auth_required = TaskState::AuthRequired;
    let valid_targets = [TaskState::Working, TaskState::Failed, TaskState::Canceled];

    for state in all_states() {
        let expected = valid_targets.contains(&state);
        assert_eq!(
            auth_required.can_transition_to(&state),
            expected,
            "AuthRequired -> {:?} should be {}",
            state,
            expected
        );
    }
}

#[test]
fn unknown_cannot_transition() {
    let unknown = TaskState::Unknown;
    for state in all_states() {
        assert!(
            !unknown.can_transition_to(&state),
            "Unknown should not transition to {:?}",
            state
        );
    }
}
