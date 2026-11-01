use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId, EventOutboxId};
use systemprompt_models::brain::{
    BrainEnvelope, BrainEvent, BrainEventKind, FactCandidate, FactSource, KindDecl, SourceNode,
    SourceSpan, SourceText,
};

use crate::fixtures::{at, kind, subject, user};

pub(crate) fn kind_decl() -> KindDecl {
    serde_json::from_value(serde_json::json!({
        "kind": "commitment",
        "subject": "client",
        "fields": {
            "owner": { "type": "text", "required": true, "max_len": 10 },
            "status": { "type": "enum", "values": ["open", "closed"] },
            "due": { "type": "date" },
            "amount": { "type": "number" },
            "partner": { "type": "entity", "ref": "client" }
        },
        "statement": "{subject} owes {owner}",
        "identity": ["subject", "owner"],
        "materiality": { "require": ["owner"] },
        "extractor": { "type": "model", "prompt": "Find commitments.", "prompt_version": "v3" }
    }))
    .expect("kind decl")
}

pub(crate) fn valid_candidate() -> FactCandidate {
    FactCandidate::builder(
        kind("commitment"),
        subject("client:acme"),
        span_source(None),
    )
    .field("owner", serde_json::json!("billing"))
    .field("status", serde_json::json!("open"))
    .field("due", serde_json::json!("2026-11-30"))
    .field("amount", serde_json::json!(12))
    .field("partner", serde_json::json!("client:globex"))
    .extractor("test")
    .build()
}

pub(crate) fn span_source(span: Option<SourceSpan>) -> FactSource {
    FactSource::Document {
        document_id: DocumentId::new("doc-1"),
        revision_id: DocumentRevisionId::new("rev-1"),
        span,
    }
}

pub(crate) fn span(node: &str, hash8: &str, find: &str) -> SourceSpan {
    SourceSpan {
        node_id: DocumentNodeId::new(node),
        hash8: hash8.to_owned(),
        find: find.to_owned(),
        nth: 0,
    }
}

pub(crate) fn source_text() -> SourceText {
    SourceText {
        text: "Acme owes billing a refund.".to_owned(),
        nodes: vec![SourceNode {
            node_id: DocumentNodeId::new("n1"),
            hash8: "abcd1234".to_owned(),
            text: "Acme owes billing a refund.".to_owned(),
        }],
    }
}

pub(crate) fn envelope(kind_name: &str) -> BrainEnvelope {
    let kind = BrainEventKind::new(kind_name).expect("event kind");
    BrainEnvelope::new(
        EventOutboxId::new("outbox-1"),
        BrainEvent::new(kind, user("u1"), at(0)),
        at(0),
    )
    .expect("envelope")
}
