use std::str::FromStr;

use chrono::Utc;
use systemprompt_identifiers::{
    AgentRunId, DocumentCommentId, DocumentId, DocumentRevisionId, SessionId, UserId,
};
use systemprompt_models::brain::{FactSource, Pathway, SourceClass};
use systemprompt_models::notifications::SubjectRef;
use systemprompt_models::notifications::receipt::ReceiptState;

fn sources() -> Vec<(FactSource, SourceClass, &'static str)> {
    let now = Utc::now();
    vec![
        (
            FactSource::Document {
                document_id: DocumentId::new("d1"),
                revision_id: DocumentRevisionId::new("r1"),
                span: None,
            },
            SourceClass::Document,
            "doc:d1@r1",
        ),
        (
            FactSource::Comment {
                document_id: DocumentId::new("d2"),
                comment_id: DocumentCommentId::new("c1"),
            },
            SourceClass::Comment,
            "comment:c1",
        ),
        (
            FactSource::Session {
                session_id: SessionId::new("s1"),
                owner: UserId::new("u1"),
            },
            SourceClass::Session,
            "session:s1",
        ),
        (
            FactSource::EnteredBy {
                user_id: UserId::new("u2"),
            },
            SourceClass::EnteredBy,
            "entered:u2",
        ),
        (
            FactSource::External {
                system: "jira".to_owned(),
                reference: "SP-1".to_owned(),
            },
            SourceClass::External,
            "jira:SP-1",
        ),
        (
            FactSource::Run {
                run_id: AgentRunId::new("run1"),
            },
            SourceClass::Run,
            "run:run1",
        ),
        (
            FactSource::Aggregate {
                pathway: Pathway::Entered,
                window_start: now,
                window_end: now,
            },
            SourceClass::Aggregate,
            "aggregate:entered",
        ),
    ]
}

#[test]
fn every_source_names_its_class_and_label() {
    for (source, class, label) in sources() {
        assert_eq!(source.class(), class);
        assert_eq!(source.label(), label);
        assert_eq!(
            serde_json::to_value(class).unwrap(),
            class.as_str(),
            "{label}"
        );
    }
}

#[test]
fn only_documents_comments_and_external_systems_gate_visibility() {
    let subjects: Vec<Option<SubjectRef>> = sources()
        .into_iter()
        .map(|(source, _, _)| source.visibility_subject())
        .collect();
    assert_eq!(subjects[0], Some(SubjectRef::new("document", "d1")));
    assert_eq!(subjects[1], Some(SubjectRef::new("document", "d2")));
    assert_eq!(subjects[4], Some(SubjectRef::new("external:jira", "SP-1")));
    for index in [2, 3, 5, 6] {
        assert_eq!(subjects[index], None, "source {index}");
    }
    let documents: Vec<Option<String>> = sources()
        .into_iter()
        .map(|(source, _, _)| source.document_id().map(|id| id.as_str().to_owned()))
        .collect();
    assert_eq!(documents[0].as_deref(), Some("d1"));
    assert_eq!(documents[1].as_deref(), Some("d2"));
    assert!(documents[2..].iter().all(Option::is_none));
}

#[test]
fn receipt_states_round_trip_through_their_names() {
    for state in [
        ReceiptState::Delivered,
        ReceiptState::Seen,
        ReceiptState::Read,
        ReceiptState::Acted,
        ReceiptState::Escalated,
    ] {
        assert_eq!(ReceiptState::from_str(state.as_str()).unwrap(), state);
        assert_eq!(state.to_string(), state.as_str());
        assert_eq!(serde_json::to_value(state).unwrap(), state.as_str());
    }
    let err = ReceiptState::from_str("archived").unwrap_err();
    assert!(err.to_string().contains("archived"), "{err}");
}
