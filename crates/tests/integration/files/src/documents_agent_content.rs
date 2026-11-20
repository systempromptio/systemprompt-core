//! The sticky `documents.agent_content` flag: set in the transaction that
//! stores an agent's revision, an agent's create, an accepted agent proposal
//! (even one that lands as the reviewer) and the copy of a flagged document;
//! never set by a person's own work and never cleared.

use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::agents::AgentCap;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitRequest, CopyRequest, ReviewRequest,
};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::AccessLevel;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_agent_fixture::{agent_harness, as_agent, document, edit};
use crate::documents_proposals::{grant, memo, outline, propose, rewrite};
use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, api_author, harness, person, trace,
};

async fn edit_harness() -> Harness {
    agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await
}

async fn flagged(h: &Harness, id: &DocumentId) -> bool {
    h.service.agent_content(id).await.expect("agent_content")
}

async fn copy_of(h: &Harness, who: &AuthenticatedUser, source: &DocumentId) -> DocumentId {
    h.service
        .copy(
            who,
            CopyRequest {
                source: source.clone(),
                name: None,
                folder: None,
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("copy")
        .0
        .id
}

#[tokio::test]
async fn a_person_s_document_edits_and_copies_stay_unflagged() {
    let h = edit_harness().await;
    let owner = person("content-person");
    let id = document(&h, &owner).await;
    assert!(!flagged(&h, &id).await);
    edit(&h, &owner, &id, "a person's edit\n")
        .await
        .expect("edit");
    assert!(!flagged(&h, &id).await);
    let copy = copy_of(&h, &owner, &id).await;
    assert!(!flagged(&h, &copy).await);
}

#[tokio::test]
async fn an_agent_revision_sets_the_flag_and_a_later_person_edit_never_clears_it() {
    let h = edit_harness().await;
    let owner = person("content-agent-edit");
    let id = document(&h, &owner).await;
    as_agent(edit(&h, &owner, &id, "the agent's edit\n"))
        .await
        .expect("agent edit");
    assert!(flagged(&h, &id).await);
    edit(&h, &owner, &id, "the person again\n")
        .await
        .expect("person edit");
    assert!(flagged(&h, &id).await);
}

#[tokio::test]
async fn an_agent_create_and_the_copy_of_a_flagged_document_carry_the_flag() {
    let h = edit_harness().await;
    let owner = person("content-agent-create");
    let id = as_agent(document(&h, &owner)).await;
    assert!(flagged(&h, &id).await);
    let copy = copy_of(&h, &owner, &id).await;
    assert!(flagged(&h, &copy).await);
    let copy_of_copy = copy_of(&h, &owner, &copy).await;
    assert!(flagged(&h, &copy_of_copy).await);
}

#[tokio::test]
async fn an_accepted_agent_proposal_sets_the_flag_when_it_lands_as_the_reviewer() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("content-review-owner");
    let proposer = person("content-review-proposer");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &proposer, AccessLevel::Comment).await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "alpha", "ALPHA")],
    )
    .await
    .expect("propose");
    assert!(flagged(&h, &document).await, "storing the proposal flags");

    let pool = test_db_pool().await;
    sqlx::query("UPDATE documents SET agent_content = false WHERE id = $1::uuid")
        .bind(document.as_str())
        .execute(pool.write_pool().as_ref())
        .await
        .expect("clear the flag as a pre-flag proposal would leave it");
    let head = outline(&h, &owner, &document, None).await;
    h.service
        .commit(
            &owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: None,
                body: CommitBody::Ops(vec![rewrite(&head, "gamma", "GAMMA")]),
                author: api_author(),
                message: None,
                lock_holder: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("owner edit");
    assert!(!column(&document).await);

    let accepted = h
        .service
        .accept_proposal(
            &owner,
            ReviewRequest {
                proposal_id: proposed.proposal.id.clone(),
                note: None,
                reviewer: api_author(),
            },
            &trace(),
        )
        .await
        .expect("accept");
    assert!(accepted.rebased, "the head moved, so the reviewer lands it");
    assert_eq!(accepted.revision.author_user_id, owner.id);
    assert!(
        column(&document).await,
        "landing the agent's proposal sets the column"
    );
}

#[tokio::test]
async fn a_document_an_agent_wrote_before_the_column_reads_as_agent_content() {
    let h = edit_harness().await;
    let owner = person("content-legacy");
    let id = document(&h, &owner).await;
    assert!(!flagged(&h, &id).await);
    let pool = test_db_pool().await;
    sqlx::query("UPDATE document_revisions SET actor_kind = 'agent' WHERE document_id = $1::uuid")
        .bind(id.as_str())
        .execute(pool.write_pool().as_ref())
        .await
        .expect("plant a pre-044 agent revision");
    assert!(!column(&id).await, "the column itself is never backfilled");
    assert!(flagged(&h, &id).await);
}

async fn column(id: &DocumentId) -> bool {
    let pool = test_db_pool().await;
    sqlx::query_scalar::<_, bool>("SELECT agent_content FROM documents WHERE id = $1::uuid")
        .bind(id.as_str())
        .fetch_one(pool.write_pool().as_ref())
        .await
        .expect("agent_content column")
}
