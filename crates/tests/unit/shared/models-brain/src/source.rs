use chrono::{TimeZone, Utc};
use serde_json::json;
use systemprompt_identifiers::{
    AgentRunId, DocumentCommentId, DocumentId, DocumentNodeId, DocumentRevisionId, SessionId,
    UserId,
};
use systemprompt_models::brain::{FactSource, Pathway, SourceClass, SourceSpan};
use systemprompt_models::notifications::SubjectRef;

const DOC: &str = "0b9d8a52-3f1e-4c7a-9e2d-5a6b7c8d9e0f";
const REV: &str = "1c2d3e4f-5a6b-4c7d-8e9f-0a1b2c3d4e5f";
const COMMENT: &str = "2d3e4f5a-6b7c-4d8e-9f0a-1b2c3d4e5f60";
const RUN: &str = "3e4f5a6b-7c8d-4e9f-8a1b-2c3d4e5f6071";

fn round_trip(source: &FactSource) {
    let encoded = serde_json::to_value(source).unwrap_or_else(|e| panic!("encode: {e}"));
    let decoded: FactSource =
        serde_json::from_value(encoded).unwrap_or_else(|e| panic!("decode: {e}"));
    assert_eq!(&decoded, source);
}

fn document() -> FactSource {
    FactSource::Document {
        document_id: DocumentId::new(DOC),
        revision_id: DocumentRevisionId::new(REV),
        span: Some(SourceSpan {
            node_id: DocumentNodeId::new("n-9"),
            hash8: "0a1b2c3d".to_owned(),
            find: "shall pay".to_owned(),
            nth: 1,
        }),
    }
}

#[test]
fn every_source_round_trips() {
    let at = Utc
        .with_ymd_and_hms(2026, 10, 1, 0, 0, 0)
        .single()
        .unwrap_or_default();
    let sources = [
        document(),
        FactSource::Comment {
            document_id: DocumentId::new(DOC),
            comment_id: DocumentCommentId::new(COMMENT),
        },
        FactSource::Session {
            session_id: SessionId::new("sess_1"),
            owner: UserId::new("u-1"),
        },
        FactSource::EnteredBy {
            user_id: UserId::new("u-1"),
        },
        FactSource::External {
            system: "crm".to_owned(),
            reference: "deal/7".to_owned(),
        },
        FactSource::Run {
            run_id: AgentRunId::new(RUN),
        },
        FactSource::Aggregate {
            pathway: Pathway::DocumentRevision,
            window_start: at,
            window_end: at,
        },
    ];
    for source in &sources {
        round_trip(source);
    }
}

#[test]
fn document_source_wire_shape() {
    let encoded = serde_json::to_value(document()).unwrap_or_else(|e| panic!("encode: {e}"));
    assert_eq!(encoded["type"], json!("document"));
    assert_eq!(encoded["span"]["nth"], json!(1));
    let entered = serde_json::to_value(FactSource::EnteredBy {
        user_id: UserId::new("u-1"),
    })
    .unwrap_or_else(|e| panic!("encode: {e}"));
    assert_eq!(entered, json!({"type": "entered_by", "user_id": "u-1"}));
}

#[test]
fn unknown_source_fields_are_refused() {
    let raw = json!({"type": "entered_by", "user_id": "u-1", "extra": 1});
    assert!(serde_json::from_value::<FactSource>(raw).is_err());
}

#[test]
fn visibility_subject_names_the_gating_resource() {
    assert_eq!(
        document().visibility_subject(),
        Some(SubjectRef::new("document", DOC))
    );
    let comment = FactSource::Comment {
        document_id: DocumentId::new(DOC),
        comment_id: DocumentCommentId::new(COMMENT),
    };
    assert_eq!(
        comment.visibility_subject(),
        Some(SubjectRef::new("document", DOC))
    );
    let external = FactSource::External {
        system: "crm".to_owned(),
        reference: "deal/7".to_owned(),
    };
    assert_eq!(
        external.visibility_subject(),
        Some(SubjectRef::new("external:crm", "deal/7"))
    );
    assert_eq!(
        FactSource::EnteredBy {
            user_id: UserId::new("u")
        }
        .visibility_subject(),
        None
    );
    assert_eq!(document().class(), SourceClass::Document);
}
