//! A document landing that moves the head and a resolved comment each append
//! exactly one brain fact on the write's own connection; a proposal's
//! unlanded revision appends none, an unavailable sink records nothing and
//! the write still commits, and a failing sink rolls the landing back.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_events::services::durable::{DurableOutbox, ReportingFact, SseEvent};
use systemprompt_extension::{BrainSink, BrainSinkError, DynBrainSink};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::CommentAnchor;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommentRequest, CommitOutcome, CommitRequest, ResolveCommentRequest,
};
use systemprompt_identifiers::{Actor, DocumentId, DocumentRevisionId, EventOutboxId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::brain::{BRAIN_CONSUMER, BRAIN_EVENT_VERSION, BrainEvent, BrainEventKind};
use systemprompt_models::documents::EditOp;
use systemprompt_test_fixtures::{test_db_pool, unique_instance};
use systemprompt_traits::RepositoryError;
use tempfile::TempDir;

use crate::documents_proposals::propose;
use crate::documents_service_fixture::{
    GroupSource, Harness, NoBrain, RoleDirectory, allow, api_author, create, harness_wired,
    person, trace,
};

struct RecordingBrainSink {
    outbox: DurableOutbox,
}

#[async_trait]
impl BrainSink for RecordingBrainSink {
    fn is_available(&self) -> bool {
        true
    }

    async fn emit(
        &self,
        conn: &mut sqlx::PgConnection,
        event: &BrainEvent,
    ) -> Result<EventOutboxId, BrainSinkError> {
        let fact = ReportingFact {
            consumer: BRAIN_CONSUMER.to_owned(),
            kind: event.kind.as_str().to_owned(),
            version: BRAIN_EVENT_VERSION,
            data: event,
        };
        self.outbox
            .append_on(conn, &Actor::user(event.actor.clone()), SseEvent::FactOnly, &fact)
            .await
            .map_err(|_| {
                BrainSinkError::Repository(RepositoryError::invalid_argument(
                    "fact",
                    "outbox append failed",
                ))
            })
    }
}

struct FailingBrainSink;

#[async_trait]
impl BrainSink for FailingBrainSink {
    fn is_available(&self) -> bool {
        true
    }

    async fn emit(
        &self,
        _conn: &mut sqlx::PgConnection,
        _event: &BrainEvent,
    ) -> Result<EventOutboxId, BrainSinkError> {
        Err(BrainSinkError::Repository(RepositoryError::invalid_argument(
            "fact",
            "refused by the test sink",
        )))
    }
}

async fn harness_with(brain: DynBrainSink) -> Harness {
    harness_on_storage(brain, Arc::new(TempDir::new().expect("tempdir"))).await
}

async fn harness_on_storage(brain: DynBrainSink, storage: Arc<TempDir>) -> Harness {
    let db = test_db_pool().await;
    harness_wired(
        db,
        (allow(), GroupSource::new(&[]), RoleDirectory::default()),
        storage,
        DocumentsConfig::default(),
        (None, brain),
    )
}

async fn recording() -> Harness {
    let db = test_db_pool().await;
    let outbox = DurableOutbox::new(db.write_pool().as_ref().clone(), unique_instance());
    harness_with(Arc::new(RecordingBrainSink { outbox })).await
}

async fn text_document(h: &Harness, owner: &AuthenticatedUser) -> DocumentId {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "alpha beta\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

fn set_text(document: &DocumentId, text: &str) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(vec![EditOp::SetText {
            text: text.to_owned(),
        }]),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    }
}

async fn commit(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    text: &str,
) -> Result<DocumentRevisionId, DocumentsError> {
    match h
        .service
        .commit(owner, set_text(document, text), &trace())
        .await?
    {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => {
            Ok(landed.revision.id)
        },
        CommitOutcome::Conflict { .. } => panic!("unexpected conflict"),
    }
}

async fn head(h: &Harness, owner: &AuthenticatedUser, document: &DocumentId) -> DocumentRevisionId {
    h.service
        .project(owner, document, None, &trace())
        .await
        .expect("project")
        .0
        .id
}

async fn brain_facts(kind: &str, document: &DocumentId) -> Vec<serde_json::Value> {
    let db = test_db_pool().await;
    sqlx::query_scalar(
        "SELECT fact->'data' FROM event_outbox \
         WHERE consumer = $1 AND fact->>'kind' = $2 \
         AND fact->'data'->'payload'->>'document_id' = $3",
    )
    .bind(BRAIN_CONSUMER)
    .bind(kind)
    .bind(document.as_str())
    .fetch_all(db.write_pool().as_ref())
    .await
    .expect("outbox facts")
}

async fn landed_facts_for(document: &DocumentId, revision: &DocumentRevisionId) -> usize {
    brain_facts(BrainEventKind::REVISION_LANDED, document)
        .await
        .iter()
        .filter(|data| data["payload"]["revision_id"] == revision.as_str())
        .count()
}

#[tokio::test]
async fn a_head_moving_commit_appends_one_revision_landed_fact() {
    let h = recording().await;
    let owner = person("brain-land-owner");
    let document = text_document(&h, &owner).await;
    let parent = head(&h, &owner, &document).await;

    let revision = commit(&h, &owner, &document, "alpha gamma\n")
        .await
        .expect("commit");

    let facts: Vec<serde_json::Value> = brain_facts(BrainEventKind::REVISION_LANDED, &document)
        .await
        .into_iter()
        .filter(|data| data["payload"]["revision_id"] == revision.as_str())
        .collect();
    assert_eq!(facts.len(), 1, "exactly one landed fact for the commit");
    let event: BrainEvent = serde_json::from_value(facts[0].clone()).expect("brain event");
    assert_eq!(event.kind.as_str(), BrainEventKind::REVISION_LANDED);
    assert_eq!(event.actor, owner.id);
    assert!(!event.actor_is_agent);
    assert_eq!(event.payload["document_id"], document.as_str());
    assert_eq!(event.payload["parent_revision_id"], parent.as_str());
    assert_eq!(event.payload["author"], owner.id.as_str());
    assert_eq!(event.payload["actor_kind"], "user");
}

#[tokio::test]
async fn a_proposed_revision_appends_no_fact() {
    let h = recording().await;
    let owner = person("brain-draft-owner");
    let document = text_document(&h, &owner).await;

    let proposed = propose(
        &h,
        &owner,
        &document,
        vec![EditOp::SetText {
            text: "alpha delta\n".to_owned(),
        }],
    )
    .await
    .expect("propose");

    assert_eq!(landed_facts_for(&document, &proposed.revision.id).await, 0);
}

#[tokio::test]
async fn resolving_a_comment_appends_one_comment_resolved_fact() {
    let h = recording().await;
    let owner = person("brain-resolve-owner");
    let document = text_document(&h, &owner).await;
    let (_, projection) = h
        .service
        .project(&owner, &document, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line")
        .id
        .clone();
    let comment = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                document: document.clone(),
                anchor: Some(CommentAnchor {
                    node: node.clone(),
                    find: Some("beta".to_owned()),
                    nth: None,
                }),
                range: None,
                body: "settle this".to_owned(),
                mentions: Vec::new(),
                parent_id: None,
                mention_options: vec![],
            },
            &trace(),
        )
        .await
        .expect("comment");

    let resolved = h
        .service
        .resolve_comment(
            &owner,
            ResolveCommentRequest {
                document: document.clone(),
                comment: comment.id.clone(),
                answering: None,
            },
            &trace(),
        )
        .await
        .expect("resolve");
    assert!(resolved.resolved_at.is_some());

    let facts = brain_facts(BrainEventKind::COMMENT_RESOLVED, &document).await;
    assert_eq!(facts.len(), 1, "one resolved fact");
    let payload = &facts[0]["payload"];
    assert_eq!(payload["comment_id"], comment.id.as_str());
    assert_eq!(payload["resolved_by"], owner.id.as_str());
    assert_eq!(payload["node_id"], node.as_str());
    assert_eq!(payload["anchor_find"], "beta");
}

#[tokio::test]
async fn an_unavailable_sink_records_nothing_and_the_commit_lands() {
    let h = harness_with(Arc::new(NoBrain)).await;
    let owner = person("brain-off-owner");
    let document = text_document(&h, &owner).await;

    let revision = commit(&h, &owner, &document, "alpha off\n")
        .await
        .expect("commit");

    assert_eq!(head(&h, &owner, &document).await, revision);
    assert!(
        brain_facts(BrainEventKind::REVISION_LANDED, &document)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_failing_sink_rolls_the_landing_back() {
    let storage = Arc::new(TempDir::new().expect("tempdir"));
    let setup = harness_on_storage(Arc::new(NoBrain), Arc::clone(&storage)).await;
    let h = harness_on_storage(Arc::new(FailingBrainSink), storage).await;
    let owner = person("brain-fail-owner");
    let document = text_document(&setup, &owner).await;
    let before = head(&h, &owner, &document).await;

    let refused = commit(&h, &owner, &document, "alpha refused\n").await;

    assert!(
        matches!(refused, Err(DocumentsError::Brain(_))),
        "the sink failure surfaces: {refused:?}"
    );
    assert_eq!(head(&h, &owner, &document).await, before, "head unchanged");
}
