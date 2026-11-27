use systemprompt_engine::run::Reply;
use systemprompt_harness::command::{
    AnswerSource, Answers, Decider, PermissionMode, UnusedAnswers,
};

const REJECT: Reply = Reply::Reject { feedback: None };

fn script() -> Answers {
    Answers::parse(
        r#"{
            "permissions": [
                {"type": "once"},
                {"type": "always"},
                {"type": "reject", "feedback": "not this one"}
            ],
            "questions": [[["Yes"], ["A", "B"]], null]
        }"#,
    )
    .unwrap()
}

#[test]
fn the_mode_names_parse_and_print() {
    for (name, mode) in [
        ("ask", PermissionMode::Ask),
        ("allow", PermissionMode::Allow),
        ("deny", PermissionMode::Deny),
    ] {
        assert_eq!(PermissionMode::parse(name), Some(mode));
        assert_eq!(mode.as_str(), name);
        assert_eq!(serde_json::to_value(mode).unwrap(), name);
    }
    assert_eq!(PermissionMode::parse("Allow"), None);
    assert_eq!(PermissionMode::parse(""), None);
    assert_eq!(PermissionMode::default(), PermissionMode::Ask);
}

#[test]
fn allow_approves_every_request_once_and_leaves_the_script_alone() {
    let mut decider = Decider::new(PermissionMode::Allow, script());
    for _ in 0..5 {
        assert_eq!(decider.permission(), (Reply::Once, AnswerSource::Mode));
    }
    assert_eq!(decider.unused().permissions, 3);
}

#[test]
fn deny_rejects_every_request_without_feedback() {
    let mut decider = Decider::new(PermissionMode::Deny, script());
    for _ in 0..5 {
        assert_eq!(decider.permission(), (REJECT, AnswerSource::Mode));
    }
    assert_eq!(decider.unused().permissions, 3);
}

#[test]
fn ask_takes_the_scripted_replies_in_order_then_rejects() {
    let mut decider = Decider::new(PermissionMode::Ask, script());
    assert_eq!(decider.permission(), (Reply::Once, AnswerSource::Scripted));
    assert_eq!(
        decider.permission(),
        (Reply::Always, AnswerSource::Scripted)
    );
    assert_eq!(
        decider.permission(),
        (
            Reply::Reject {
                feedback: Some("not this one".to_owned())
            },
            AnswerSource::Scripted
        )
    );
    assert_eq!(decider.permission(), (REJECT, AnswerSource::Unanswered));
    assert_eq!(decider.permission(), (REJECT, AnswerSource::Unanswered));
}

#[test]
fn ask_without_a_script_rejects() {
    let mut decider = Decider::new(PermissionMode::Ask, Answers::default());
    assert_eq!(decider.permission(), (REJECT, AnswerSource::Unanswered));
}

#[test]
fn questions_take_the_script_in_every_mode_and_are_dismissed_without_one() {
    for mode in [
        PermissionMode::Ask,
        PermissionMode::Allow,
        PermissionMode::Deny,
    ] {
        let mut decider = Decider::new(mode, script());
        assert_eq!(
            decider.question(),
            (
                Some(vec![
                    vec!["Yes".to_owned()],
                    vec!["A".to_owned(), "B".to_owned()]
                ]),
                AnswerSource::Scripted
            )
        );
        assert_eq!(decider.question(), (None, AnswerSource::Scripted));
        assert_eq!(decider.question(), (None, AnswerSource::Unanswered));
    }
}

#[test]
fn unused_counts_what_the_run_did_not_consume() {
    let mut decider = Decider::new(PermissionMode::Ask, script());
    assert_eq!(
        decider.unused(),
        UnusedAnswers {
            permissions: 3,
            questions: 2
        }
    );
    decider.permission();
    decider.question();
    assert_eq!(
        decider.unused(),
        UnusedAnswers {
            permissions: 2,
            questions: 1
        }
    );
}

#[test]
fn an_answers_file_may_leave_either_list_out_and_refuses_unknown_keys() {
    assert_eq!(Answers::parse("{}").unwrap(), Answers::default());
    let only_permissions = Answers::parse(r#"{"permissions":[{"type":"once"}]}"#).unwrap();
    assert_eq!(only_permissions.permissions.len(), 1);
    assert!(only_permissions.questions.is_empty());
    assert!(Answers::parse(r#"{"permission":[]}"#).is_err());
    assert!(Answers::parse(r#"{"permissions":[{"type":"sometimes"}]}"#).is_err());
}
