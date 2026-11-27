//! The client sends each route as the instance expects it and decodes what
//! the instance answers.

use serde_json::{Map, json};
use systemprompt_harness::members::brain::FactQuery;
use systemprompt_identifiers::{BrainFactId, BrainProposalId, FactKindId};
use systemprompt_models::brain::client::{DisputeOutcome, ReplayRequest, ResolveRequest};
use systemprompt_models::brain::{
    EnteredFact, EnteredStatus, FactScope, FactStatus, SubjectKey, WithheldReason,
};

use crate::members::brain::support::{
    AT, DISPUTE, FACT, FakeInstance, FakeResponse, OUTBOX, PROPOSAL, fact_json, revision_json,
};

fn fact() -> BrainFactId {
    BrainFactId::try_new(FACT).unwrap()
}

#[tokio::test]
async fn a_listing_sends_its_filters_and_reads_the_page() {
    let fake = FakeInstance::start(|_| {
        FakeResponse::json(
            200,
            &json!({ "facts": [fact_json(FACT, "confirmed")], "next": "c1" }),
        )
    })
    .await;
    let query = FactQuery {
        kind: Some(FactKindId::try_new("client_contact").unwrap()),
        subject: Some(SubjectKey::try_new("client", "acme").unwrap()),
        statuses: vec![FactStatus::Confirmed, FactStatus::Stale],
        limit: Some(5),
        project: Some("sales".to_owned()),
        as_of: None,
        after: Some("c0".to_owned()),
    };
    let page = fake.brain().facts(&query).await.unwrap();
    assert_eq!(page.facts[0].id, fact());
    assert_eq!(page.facts[0].status, FactStatus::Confirmed);
    assert_eq!(page.next.as_deref(), Some("c1"));
    let sent = fake.last();
    assert_eq!(sent.method, "GET");
    assert_eq!(sent.path(), "/api/v1/brain/facts");
    assert_eq!(
        sent.query(),
        "kind=client_contact&subject=client%3Aacme&status=confirmed%2Cstale&limit=5&project=sales&after=c0"
    );
    assert_eq!(
        sent.header("authorization"),
        Some("Bearer header.claims.sig1")
    );
    assert_eq!(sent.header("x-systemprompt-client"), Some("harness"));
}

#[tokio::test]
async fn an_empty_query_sends_no_parameters() {
    let fake =
        FakeInstance::start(|_| FakeResponse::json(200, &json!({ "facts": [], "next": null })))
            .await;
    let page = fake.brain().facts(&FactQuery::default()).await.unwrap();
    assert_eq!(page.facts.len(), 0);
    assert_eq!(fake.last().target, "/api/v1/brain/facts");
}

#[tokio::test]
async fn reads_of_one_fact_go_to_its_own_routes() {
    let fake = FakeInstance::start(|request| match request.path() {
        "/api/v1/brain/facts/m_6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f/timeline" => {
            FakeResponse::json(200, &json!([revision_json("confirmed", "confirmed")]))
        },
        "/api/v1/brain/facts/m_6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f/why" => FakeResponse::json(
            200,
            &json!({
                "fact": fact_json(FACT, "stale"),
                "terms": { "base": 0.5, "subject": 0.3, "workflow": 0.0, "recency": 0.1, "usage": 0.0, "total": 0.9 },
                "withheld": "stale"
            }),
        ),
        "/api/v1/brain/subjects/client/acme" => {
            FakeResponse::json(200, &json!({ "facts": [fact_json(FACT, "confirmed")], "next": null }))
        },
        _ => FakeResponse::json(200, &fact_json(FACT, "confirmed")),
    })
    .await;
    let brain = fake.brain();
    assert_eq!(brain.fact(&fact()).await.unwrap().revision, 3);
    assert_eq!(brain.timeline(&fact()).await.unwrap()[0].revision, 4);
    let why = brain.why(&fact(), Some("sales")).await.unwrap();
    assert!((why.terms.total - 0.9).abs() < f64::EPSILON);
    assert_eq!(why.withheld.map(WithheldReason::as_str), Some("stale"));
    assert_eq!(fake.last().query(), "project=sales");
    let subject = SubjectKey::try_new("client", "acme").unwrap();
    let page = brain.subject(&subject, Some(10), None).await.unwrap();
    assert_eq!(page.facts.len(), 1);
    assert_eq!(fake.last().query(), "limit=10");
}

#[tokio::test]
async fn an_entered_fact_is_posted_as_written_and_answers_its_outbox_event() {
    let fake =
        FakeInstance::start(|_| FakeResponse::json(202, &json!({ "outbox_id": OUTBOX }))).await;
    let mut fields = Map::new();
    fields.insert("contact".to_owned(), json!("Jane Doe"));
    let entered = EnteredFact {
        kind: FactKindId::try_new("client_contact").unwrap(),
        subject: SubjectKey::try_new("client", "acme").unwrap(),
        fields,
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Proposed,
        observed_at: None,
    };
    let outbox = fake.brain().enter(&entered).await.unwrap();
    assert_eq!(outbox.to_string(), OUTBOX);
    let sent = fake.last();
    assert_eq!(
        (sent.method.as_str(), sent.path()),
        ("POST", "/api/v1/brain/facts")
    );
    assert_eq!(
        sent.json(),
        json!({
            "kind": "client_contact",
            "subject": { "kind": "client", "id": "acme" },
            "fields": { "contact": "Jane Doe" },
            "scope": { "level": "org" },
            "topics": [],
            "status": "proposed",
            "observed_at": null
        })
    );
}

#[tokio::test]
async fn governing_a_fact_posts_to_its_action_and_reads_the_revision() {
    let fake = FakeInstance::start(|request| match request.path().rsplit('/').next() {
        Some("dispute") => FakeResponse::json(201, &json!({ "dispute_id": DISPUTE })),
        Some("erase") => FakeResponse::status(204),
        _ => FakeResponse::json(200, &revision_json("pinned", "confirmed")),
    })
    .await;
    let brain = fake.brain();
    let base = format!("/api/v1/brain/facts/{FACT}");
    brain.confirm(&fact()).await.unwrap();
    assert_eq!(fake.last().path(), format!("{base}/confirm"));
    assert_eq!(fake.last().body.len(), 0);
    brain.pin(&fact()).await.unwrap();
    assert_eq!(fake.last().path(), format!("{base}/pin"));
    brain.unpin(&fact()).await.unwrap();
    assert_eq!(fake.last().path(), format!("{base}/unpin"));
    brain.retract(&fact(), "").await.unwrap();
    assert_eq!(fake.last().json(), json!({ "reason": "" }));
    brain.erase(&fact()).await.unwrap();
    assert_eq!(fake.last().path(), format!("{base}/erase"));
    let dispute = brain.dispute(&fact(), "the contact changed").await.unwrap();
    assert_eq!(dispute.to_string(), DISPUTE);
    assert_eq!(
        fake.last().json(),
        json!({ "reason": "the contact changed" })
    );
    let mut fields = Map::new();
    fields.insert("role".to_owned(), json!(null));
    brain.edit_fields(&fact(), fields).await.unwrap();
    let sent = fake.last();
    assert_eq!(
        (sent.method.as_str(), sent.path()),
        ("PATCH", format!("{base}/fields").as_str())
    );
    assert_eq!(sent.json(), json!({ "fields": { "role": null } }));
}

#[tokio::test]
async fn disputes_and_proposals_are_decided_on_their_own_routes() {
    let fake = FakeInstance::start(|request| {
        if request.path() == "/api/v1/brain/proposals" {
            return FakeResponse::json(200, &json!([]));
        }
        FakeResponse::status(204)
    })
    .await;
    let brain = fake.brain();
    let dispute = serde_json::from_value(json!(DISPUTE)).unwrap();
    let resolution = ResolveRequest {
        fact_id: fact(),
        outcome: DisputeOutcome::Dismissed,
        note: None,
    };
    brain.resolve_dispute(&dispute, &resolution).await.unwrap();
    let sent = fake.last();
    assert_eq!(
        sent.path(),
        format!("/api/v1/brain/disputes/{DISPUTE}/resolve")
    );
    assert_eq!(
        sent.json(),
        json!({ "fact_id": FACT, "outcome": "dismissed" })
    );
    let proposal: BrainProposalId = serde_json::from_value(json!(PROPOSAL)).unwrap();
    brain.approve(&proposal).await.unwrap();
    assert_eq!(
        fake.last().path(),
        format!("/api/v1/brain/proposals/{PROPOSAL}/approve")
    );
    brain.reject(&proposal).await.unwrap();
    assert_eq!(
        fake.last().path(),
        format!("/api/v1/brain/proposals/{PROPOSAL}/reject")
    );
    assert_eq!(brain.proposals(Some(5)).await.unwrap().len(), 0);
    assert_eq!(fake.last().query(), "limit=5");
}

#[tokio::test]
async fn a_replay_sends_its_window_and_reads_the_report() {
    let fake = FakeInstance::start(|_| {
        FakeResponse::json(
            200,
            &json!({
                "pipeline_version": "0123456789abcdef",
                "events": 3, "failed": 0, "undecodable": 0, "truncated": false,
                "would_create": 1, "would_revise": 1, "would_dispute": 0, "unchanged": 1,
                "tokens": 120,
                "per_kind": { "client_contact": { "create": 1, "revise": 1, "dispute": 0, "unchanged": 1 } }
            }),
        )
    })
    .await;
    let proposal: BrainProposalId = serde_json::from_value(json!(PROPOSAL)).unwrap();
    let window: ReplayRequest =
        serde_json::from_value(json!({ "since": AT, "until": AT })).unwrap();
    let report = fake.brain().replay(&proposal, &window).await.unwrap();
    assert_eq!(report.events, 3);
    assert_eq!(report.per_kind.values().next().unwrap().revise, 1);
    let sent = fake.last();
    assert_eq!(
        sent.path(),
        format!("/api/v1/brain/proposals/{PROPOSAL}/replay")
    );
    assert_eq!(sent.json(), json!({ "since": AT, "until": AT }));
}
