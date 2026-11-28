use systemprompt_engine::run::Reply;
use systemprompt_engine::store::SessionId;
use systemprompt_harness::command::{
    AnswerSource, Answers, Decider, PermissionMode, UnusedAnswers, WaitingAsks,
};
use systemprompt_identifiers::engine::PermissionId;

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

fn permission(last: char) -> PermissionId {
    PermissionId::parse(&format!("per_0000000000001000000000000{last}")).unwrap()
}

fn session(last: char) -> SessionId {
    SessionId::parse(&format!("ses_0000000000001000000000000{last}")).unwrap()
}

#[test]
fn a_reply_is_routed_to_the_request_with_its_id_whatever_the_order() {
    let mut waiting = WaitingAsks::default();
    assert!(waiting.hold(&permission('1'), &session('a')));
    assert!(waiting.hold(&permission('2'), &session('b')));
    assert!(waiting.hold(&permission('3'), &session('a')));
    assert_eq!(waiting.len(), 3);

    assert_eq!(waiting.release(&permission('2')), Some(session('b')));
    assert_eq!(waiting.release(&permission('1')), Some(session('a')));
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting.release(&permission('3')), Some(session('a')));
    assert!(waiting.is_empty());
}

#[test]
fn a_request_is_held_once_and_answered_once() {
    let mut waiting = WaitingAsks::default();
    assert!(waiting.hold(&permission('1'), &session('a')));
    assert!(!waiting.hold(&permission('1'), &session('a')));
    assert_eq!(waiting.len(), 1);

    assert_eq!(waiting.release(&permission('1')), Some(session('a')));
    assert_eq!(waiting.release(&permission('1')), None);
    assert_eq!(waiting.release(&permission('9')), None);
}

#[test]
fn the_requests_left_when_the_stream_ends_come_out_in_the_order_they_were_raised() {
    let mut waiting = WaitingAsks::default();
    waiting.hold(&permission('2'), &session('b'));
    waiting.hold(&permission('1'), &session('a'));

    assert_eq!(
        waiting.drain(),
        [
            (permission('2'), session('b')),
            (permission('1'), session('a')),
        ]
    );
    assert!(waiting.is_empty());
    assert!(waiting.drain().is_empty());
}

#[test]
fn an_answer_from_the_control_stream_is_named_as_such() {
    assert_eq!(
        serde_json::to_value(AnswerSource::Control).unwrap(),
        "control"
    );
}
