use std::path::PathBuf;

use serde_json::json;
use systemprompt_engine::run::{
    EngineEvent, ErrorKind, PermissionAsk, Question, QuestionAsk, QuestionOption, Reply, RunStatus,
    SessionFailure,
};
use systemprompt_engine::store::{
    CallId, Change, DiffSummary, JsonMap, Message, MessageId, Part, PartBody, PartId, ProjectId,
    Session, SessionId, Todo, TodoPriority, TodoStatus, ToolState, UserMessage,
};
use systemprompt_harness::command::{
    AnswerSource, EventWriter, ExitStatus, Feed, PermissionMode, RunEvent, UnusedAnswers,
};
use systemprompt_identifiers::engine::{PermissionId, QuestionId};

const SESSION: &str = "ses_00000000000010000000000001";
const PROJECT: &str = "prj_00000000000010000000000002";
const MESSAGE: &str = "msg_00000000000010000000000003";
const PART: &str = "prt_00000000000010000000000004";
const PERMISSION: &str = "per_00000000000010000000000005";
const QUESTION: &str = "qst_00000000000010000000000006";

fn session_id() -> SessionId {
    SessionId::parse(SESSION).unwrap()
}

fn message_id() -> MessageId {
    MessageId::parse(MESSAGE).unwrap()
}

fn part_id() -> PartId {
    PartId::parse(PART).unwrap()
}

fn line(event: &RunEvent) -> String {
    event.encode().unwrap()
}

fn part(body: PartBody) -> Part {
    Part {
        id: part_id(),
        session_id: session_id(),
        message_id: message_id(),
        created_at: 1_700_000_000_001,
        updated_at: 1_700_000_000_002,
        body,
    }
}

fn permission_ask() -> PermissionAsk {
    PermissionAsk {
        id: PermissionId::parse(PERMISSION).unwrap(),
        session: session_id(),
        call_id: CallId::try_new("call-1").unwrap(),
        tool: "shell".to_owned(),
        permission: "shell".to_owned(),
        patterns: vec!["git status".to_owned()],
        always: vec!["git *".to_owned()],
        title: "git status".to_owned(),
        metadata: json!({ "command": "git status" }),
    }
}

fn question_ask() -> QuestionAsk {
    QuestionAsk {
        id: QuestionId::parse(QUESTION).unwrap(),
        session: session_id(),
        call_id: CallId::try_new("call-2").unwrap(),
        questions: vec![Question {
            question: "Proceed?".to_owned(),
            header: "Confirm".to_owned(),
            options: vec![QuestionOption {
                label: "Yes".to_owned(),
                description: String::new(),
            }],
            multiple: false,
        }],
    }
}

#[test]
fn started_line() {
    let event = RunEvent::Started {
        session_id: session_id(),
        directory: PathBuf::from("/work/project"),
        permission_mode: PermissionMode::Ask,
        product: "harness-tests".to_owned(),
        version: "0.0.0".to_owned(),
    };
    assert_eq!(
        line(&event),
        r#"{"type":"started","session_id":"ses_00000000000010000000000001","directory":"/work/project","permission_mode":"ask","product":"harness-tests","version":"0.0.0"}"#
    );
}

#[test]
fn session_line() {
    let session = Session {
        id: session_id(),
        project_id: ProjectId::parse(PROJECT).unwrap(),
        parent_id: None,
        directory: PathBuf::from("/work/project"),
        title: "Read a file".to_owned(),
        agent: None,
        model: None,
        permission: None,
        revert: None,
        summary: DiffSummary::default(),
        metadata: JsonMap::new(),
        created_at: 1,
        updated_at: 2,
        archived_at: None,
    };
    let created = RunEvent::of_change(Change::SessionCreated(Box::new(session.clone()))).unwrap();
    let updated = RunEvent::of_change(Change::SessionUpdated(Box::new(session))).unwrap();
    assert_eq!(created, updated);
    assert_eq!(
        line(&created),
        r#"{"type":"session","session":{"id":"ses_00000000000010000000000001","project_id":"prj_00000000000010000000000002","parent_id":null,"directory":"/work/project","title":"Read a file","agent":null,"model":null,"permission":null,"revert":null,"summary":{"additions":0,"deletions":0,"files":0},"metadata":{},"created_at":1,"updated_at":2,"archived_at":null}}"#
    );
}

#[test]
fn status_lines() {
    let idle = RunEvent::of_engine(EngineEvent::Status {
        session: session_id(),
        status: RunStatus::Idle,
    })
    .unwrap();
    assert_eq!(
        line(&idle),
        r#"{"type":"status","session_id":"ses_00000000000010000000000001","status":{"type":"idle"}}"#
    );
    let retry = RunEvent::of_engine(EngineEvent::Status {
        session: session_id(),
        status: RunStatus::Retry {
            attempt: 2,
            message: "overloaded".to_owned(),
            next_ms: 1500,
        },
    })
    .unwrap();
    assert_eq!(
        line(&retry),
        r#"{"type":"status","session_id":"ses_00000000000010000000000001","status":{"type":"retry","attempt":2,"message":"overloaded","next_ms":1500}}"#
    );
}

#[test]
fn message_lines() {
    let message = Message::User(UserMessage {
        id: message_id(),
        session_id: session_id(),
        agent: "build".to_owned(),
        model: "model-a".to_owned(),
        system: false,
        created_at: 1_700_000_000_000,
    });
    assert_eq!(
        line(&RunEvent::of_change(Change::MessageUpdated(message)).unwrap()),
        r#"{"type":"message","message":{"role":"user","id":"msg_00000000000010000000000003","session_id":"ses_00000000000010000000000001","agent":"build","model":"model-a","system":false,"created_at":1700000000000}}"#
    );
    let removed = RunEvent::of_change(Change::MessageRemoved {
        session_id: session_id(),
        message_id: message_id(),
    })
    .unwrap();
    assert_eq!(
        line(&removed),
        r#"{"type":"message_removed","session_id":"ses_00000000000010000000000001","message_id":"msg_00000000000010000000000003"}"#
    );
}

#[test]
fn part_lines() {
    let text = part(PartBody::Text {
        text: "Hello".to_owned(),
        synthetic: false,
    });
    assert_eq!(
        line(&RunEvent::of_change(Change::PartUpdated(text)).unwrap()),
        r#"{"type":"part","part":{"id":"prt_00000000000010000000000004","session_id":"ses_00000000000010000000000001","message_id":"msg_00000000000010000000000003","created_at":1700000000001,"updated_at":1700000000002,"body":{"type":"text","text":"Hello","synthetic":false}}}"#
    );
    let tool = part(PartBody::Tool {
        call_id: CallId::try_new("call-1").unwrap(),
        tool: "read".to_owned(),
        state: Box::new(ToolState::Completed {
            input: json!({ "path": "src/lib.rs" }),
            output: "fn main() {}".to_owned(),
            title: "src/lib.rs".to_owned(),
            metadata: JsonMap::new(),
            attachments: Vec::new(),
            started: 1,
            ended: 2,
        }),
    });
    assert_eq!(
        line(&RunEvent::of_change(Change::PartUpdated(tool)).unwrap()),
        r#"{"type":"part","part":{"id":"prt_00000000000010000000000004","session_id":"ses_00000000000010000000000001","message_id":"msg_00000000000010000000000003","created_at":1700000000001,"updated_at":1700000000002,"body":{"type":"tool","call_id":"call-1","tool":"read","state":{"status":"completed","input":{"path":"src/lib.rs"},"output":"fn main() {}","title":"src/lib.rs","metadata":{},"started":1,"ended":2}}}}"#
    );
    let removed = RunEvent::of_change(Change::PartRemoved {
        session_id: session_id(),
        message_id: message_id(),
        part_id: part_id(),
    })
    .unwrap();
    assert_eq!(
        line(&removed),
        r#"{"type":"part_removed","session_id":"ses_00000000000010000000000001","message_id":"msg_00000000000010000000000003","part_id":"prt_00000000000010000000000004"}"#
    );
}

#[test]
fn todos_line() {
    let event = RunEvent::of_change(Change::TodosUpdated {
        session_id: session_id(),
        todos: vec![Todo {
            content: "write tests".to_owned(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::High,
        }],
    })
    .unwrap();
    assert_eq!(
        line(&event),
        r#"{"type":"todos","session_id":"ses_00000000000010000000000001","todos":[{"content":"write tests","status":"in_progress","priority":"high"}]}"#
    );
}

#[test]
fn permission_lines() {
    let asked = RunEvent::of_engine(EngineEvent::PermissionAsked(Box::new(permission_ask())));
    assert_eq!(
        line(&asked.unwrap()),
        r#"{"type":"permission_asked","ask":{"id":"per_00000000000010000000000005","session":"ses_00000000000010000000000001","call_id":"call-1","tool":"shell","permission":"shell","patterns":["git status"],"always":["git *"],"title":"git status","metadata":{"command":"git status"}}}"#
    );
    let replied = RunEvent::PermissionReplied {
        id: PermissionId::parse(PERMISSION).unwrap(),
        session_id: session_id(),
        reply: Reply::Reject {
            feedback: Some("not now".to_owned()),
        },
        source: AnswerSource::Scripted,
    };
    assert_eq!(
        line(&replied),
        r#"{"type":"permission_replied","id":"per_00000000000010000000000005","session_id":"ses_00000000000010000000000001","reply":{"type":"reject","feedback":"not now"},"source":"scripted"}"#
    );
    let by_mode = RunEvent::PermissionReplied {
        id: PermissionId::parse(PERMISSION).unwrap(),
        session_id: session_id(),
        reply: Reply::Once,
        source: AnswerSource::Mode,
    };
    assert_eq!(
        line(&by_mode),
        r#"{"type":"permission_replied","id":"per_00000000000010000000000005","session_id":"ses_00000000000010000000000001","reply":{"type":"once"},"source":"mode"}"#
    );
}

#[test]
fn question_lines() {
    let asked = RunEvent::of_engine(EngineEvent::QuestionAsked(question_ask())).unwrap();
    assert_eq!(
        line(&asked),
        r#"{"type":"question_asked","ask":{"id":"qst_00000000000010000000000006","session":"ses_00000000000010000000000001","call_id":"call-2","questions":[{"question":"Proceed?","header":"Confirm","options":[{"label":"Yes","description":""}],"multiple":false}]}}"#
    );
    let answered = RunEvent::QuestionAnswered {
        id: QuestionId::parse(QUESTION).unwrap(),
        session_id: session_id(),
        answers: Some(vec![vec!["Yes".to_owned()]]),
        source: AnswerSource::Scripted,
    };
    assert_eq!(
        line(&answered),
        r#"{"type":"question_answered","id":"qst_00000000000010000000000006","session_id":"ses_00000000000010000000000001","answers":[["Yes"]],"source":"scripted"}"#
    );
    let dismissed = RunEvent::QuestionAnswered {
        id: QuestionId::parse(QUESTION).unwrap(),
        session_id: session_id(),
        answers: None,
        source: AnswerSource::Unanswered,
    };
    assert_eq!(
        line(&dismissed),
        r#"{"type":"question_answered","id":"qst_00000000000010000000000006","session_id":"ses_00000000000010000000000001","answers":null,"source":"unanswered"}"#
    );
}

#[test]
fn error_and_lagged_lines() {
    let error = RunEvent::of_engine(EngineEvent::SessionError {
        session: session_id(),
        error: SessionFailure {
            kind: ErrorKind::Governance,
            message: "the instance refused the call: blocked".to_owned(),
        },
    })
    .unwrap();
    assert_eq!(
        line(&error),
        r#"{"type":"error","session_id":"ses_00000000000010000000000001","kind":"governance","message":"the instance refused the call: blocked"}"#
    );
    let lagged = RunEvent::Lagged {
        feed: Feed::Store,
        missed: 7,
    };
    assert_eq!(
        line(&lagged),
        r#"{"type":"lagged","feed":"store","missed":7}"#
    );
    let engine = RunEvent::Lagged {
        feed: Feed::Engine,
        missed: 1,
    };
    assert_eq!(
        line(&engine),
        r#"{"type":"lagged","feed":"engine","missed":1}"#
    );
}

#[test]
fn finished_lines() {
    let completed = RunEvent::Finished {
        session_id: Some(session_id()),
        outcome: ExitStatus::Completed,
        exit_code: ExitStatus::Completed.code(),
        message: None,
        unused_answers: UnusedAnswers::default(),
    };
    assert_eq!(
        line(&completed),
        r#"{"type":"finished","session_id":"ses_00000000000010000000000001","outcome":"completed","exit_code":0,"message":null,"unused_answers":{"permissions":0,"questions":0}}"#
    );
    let refused = RunEvent::Finished {
        session_id: None,
        outcome: ExitStatus::NotSignedIn,
        exit_code: ExitStatus::NotSignedIn.code(),
        message: Some("no account is signed in; run login first".to_owned()),
        unused_answers: UnusedAnswers {
            permissions: 1,
            questions: 0,
        },
    };
    assert_eq!(
        line(&refused),
        r#"{"type":"finished","session_id":null,"outcome":"not_signed_in","exit_code":5,"message":"no account is signed in; run login first","unused_answers":{"permissions":1,"questions":0}}"#
    );
}

#[test]
fn changes_and_events_outside_the_stream_are_dropped() {
    let dropped_changes = [
        Change::ProjectDeleted {
            id: ProjectId::parse(PROJECT).unwrap(),
        },
        Change::SessionDeleted {
            id: session_id(),
            project_id: ProjectId::parse(PROJECT).unwrap(),
            parent_id: None,
        },
        Change::PartDelta {
            session_id: session_id(),
            message_id: message_id(),
            part_id: part_id(),
            delta: "Hel".to_owned(),
        },
    ];
    for change in dropped_changes {
        assert_eq!(RunEvent::of_change(change), None);
    }
    let dropped_events = [
        EngineEvent::PermissionResolved {
            id: PermissionId::parse(PERMISSION).unwrap(),
            session: session_id(),
            reply: Reply::Once,
        },
        EngineEvent::QuestionResolved {
            id: QuestionId::parse(QUESTION).unwrap(),
            session: session_id(),
            answered: true,
        },
        EngineEvent::McpChanged {
            directory: PathBuf::from("/work/project"),
        },
        EngineEvent::ModelsChanged,
    ];
    for event in dropped_events {
        assert_eq!(RunEvent::of_engine(event), None);
    }
}

#[test]
fn the_writer_puts_one_event_on_each_line() {
    let mut writer = EventWriter::new(Vec::new());
    let first = RunEvent::Lagged {
        feed: Feed::Store,
        missed: 1,
    };
    let second = RunEvent::Status {
        session_id: session_id(),
        status: RunStatus::Busy,
    };
    writer.emit(&first).unwrap();
    writer.emit(&second).unwrap();
    let written = String::from_utf8(writer.into_inner()).unwrap();
    assert_eq!(written, format!("{}\n{}\n", line(&first), line(&second)));
    assert!(
        written
            .lines()
            .all(|text| serde_json::from_str::<serde_json::Value>(text).is_ok())
    );
}
