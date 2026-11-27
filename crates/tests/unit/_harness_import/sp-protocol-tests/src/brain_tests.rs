//! The brain bodies read and write as the instance does, and the memory ids
//! header reads as fact references.

use serde_json::json;
use sp_protocol::wire::brain::{
    BrainFactId, DisputeOutcome, FactPage, FactWhy, MemoryIdsError, ResolveRequest, RetractRequest,
    WithheldReason, parse_memory_ids,
};
use sp_protocol::wire::headers;

const FACT: &str = "m_6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f";
const FACT2: &str = "m_7a2d3c9f-2e3b-4d4c-8f90-1b2c3d4e5f60";
const AT: &str = "2026-10-30T10:00:00Z";

fn fact_json() -> serde_json::Value {
    json!({
        "id": FACT, "kind": "client_contact",
        "subject": { "kind": "client", "id": "acme" },
        "scope": { "level": "org" },
        "statement": "Acme's primary contact is Jane Doe",
        "fields": { "contact": "Jane Doe" },
        "source": { "type": "entered_by", "user_id": "1ecce4e3-bc87-441d-b374-ea29bab88da8" },
        "topics": [], "status": "confirmed", "confirmed_by": null, "confirmed_at": null,
        "importance_base": 0.5, "observed_at": AT, "valid_from": null, "valid_to": null,
        "review_after": null, "retention_until": null, "legal_hold": false,
        "identity_hash": "ih", "statement_hash": "sh", "revision": 2, "pinned": true,
        "owner_user_id": null, "created_at": AT, "updated_at": AT
    })
}

#[test]
fn the_memory_header_names_are_the_instances() {
    assert_eq!(headers::MEMORY, "x-systemprompt-memory");
    assert_eq!(headers::MEMORY_IDS, "x-systemprompt-memory-ids");
}

#[test]
fn memory_ids_read_as_fact_and_revision_pairs() {
    let refs = parse_memory_ids(&format!("{FACT}@3, {FACT2}@12")).unwrap();
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].id, BrainFactId::try_new(FACT).unwrap());
    assert_eq!(refs[0].revision, 3);
    assert_eq!(refs[1].revision, 12);
    assert_eq!(parse_memory_ids(" ").unwrap().len(), 0);
}

#[test]
fn a_malformed_memory_id_names_its_position() {
    assert_eq!(
        parse_memory_ids(&format!("{FACT}@1,{FACT2}")),
        Err(MemoryIdsError::Shape { position: 1 })
    );
    assert!(matches!(
        parse_memory_ids("fact-1@1"),
        Err(MemoryIdsError::FactId { position: 0, .. })
    ));
    assert!(matches!(
        parse_memory_ids(&format!("{FACT}@three")),
        Err(MemoryIdsError::Revision { position: 0, .. })
    ));
}

#[test]
fn a_fact_page_and_a_why_view_read_as_the_instance_writes_them() {
    let page: FactPage =
        serde_json::from_value(json!({ "facts": [fact_json()], "next": null })).unwrap();
    assert!(page.facts[0].pinned);
    assert_eq!(page.next, None);
    let why: FactWhy = serde_json::from_value(json!({
        "fact": fact_json(),
        "terms": { "base": 0.5, "subject": 0.0, "workflow": 0.0, "recency": 0.25, "usage": 0.0, "total": 0.75 },
        "withheld": "review_due"
    }))
    .unwrap();
    assert_eq!(why.withheld, Some(WithheldReason::ReviewDue));
    assert!((why.terms.recency - 0.25).abs() < f64::EPSILON);
}

#[test]
fn request_bodies_carry_exactly_the_fields_the_instance_accepts() {
    let resolve = ResolveRequest {
        fact_id: BrainFactId::try_new(FACT).unwrap(),
        outcome: DisputeOutcome::Upheld,
        note: None,
    };
    assert_eq!(
        serde_json::to_value(&resolve).unwrap(),
        json!({ "fact_id": FACT, "outcome": "upheld" })
    );
    let noted = ResolveRequest {
        note: Some("checked with the client".to_owned()),
        ..resolve
    };
    assert_eq!(
        serde_json::to_value(&noted).unwrap()["note"],
        "checked with the client"
    );
    let retract = RetractRequest {
        reason: String::new(),
    };
    assert_eq!(
        serde_json::to_value(&retract).unwrap(),
        json!({ "reason": "" })
    );
}
