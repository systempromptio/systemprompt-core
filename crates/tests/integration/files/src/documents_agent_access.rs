use std::sync::Arc;

use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::agents::{
    AgentCap, AgentCapability, AgentRefusal, DocumentActor, act_as,
};
use systemprompt_files::documents::models::Document;
use systemprompt_files::documents::{
    DocumentAccessRepositories, DocumentAccessService, DocumentsError,
};
use systemprompt_models::auth::AuthenticatedUser;

use crate::documents_agent_fixture::{FixedPolicy, agent, agent_harness, document};
use crate::documents_service_fixture::{GroupSource, Harness, RoleDirectory, allow, person};

fn access(h: &Harness, cap: AgentCap, allows: Vec<AgentCapability>) -> DocumentAccessService {
    DocumentAccessService::new(
        DocumentAccessRepositories {
            grants: Arc::clone(&h.repos.grants),
            folders: Arc::clone(&h.repos.folders),
            documents: Arc::clone(&h.repos.documents),
            access_entities: Arc::clone(&h.access_entities),
        },
        allow(),
        Arc::new(GroupSource::new(&[])),
        Arc::new(RoleDirectory::default()),
    )
    .with_unscoped_actor(DocumentActor::Human)
    .with_agent_policy(Arc::new(FixedPolicy { cap, allows }))
}

async fn rig() -> Harness {
    agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await
}

async fn owned(h: &Harness, owner: &AuthenticatedUser) -> Document {
    let id = document(h, owner).await;
    h.repos
        .documents
        .find(&id)
        .await
        .expect("find")
        .expect("document")
}

#[tokio::test]
async fn a_capability_is_checked_only_for_an_agent_and_only_against_the_policy() {
    let h = rig().await;
    let owner = person("access-cap-owner");
    let doc = owned(&h, &owner).await;
    let strict = access(&h, AgentCap::Edit, Vec::new());
    strict
        .require_agent_capability(&owner, &doc, AgentCapability::Share)
        .await
        .expect("a person is never asked");
    let refused = act_as(
        agent(),
        strict.require_agent_capability(&owner, &doc, AgentCapability::Share),
    )
    .await
    .expect_err("not allowed");
    assert!(matches!(
        refused,
        DocumentsError::Agent(AgentRefusal::HardRule {
            capability: AgentCapability::Share,
            ..
        })
    ));
    let lenient = access(&h, AgentCap::Edit, vec![AgentCapability::Share]);
    act_as(
        agent(),
        lenient.require_agent_capability(&owner, &doc, AgentCapability::Share),
    )
    .await
    .expect("allowed");
}

#[tokio::test]
async fn a_document_is_hidden_from_an_agent_only_under_a_none_cap() {
    let h = rig().await;
    let owner = person("access-hidden-owner");
    let first = owned(&h, &owner).await;
    let second = owned(&h, &owner).await;
    let docs = [&first, &second];
    let none = access(&h, AgentCap::None, Vec::new());
    let read = access(&h, AgentCap::Read, Vec::new());
    let trace = crate::documents_service_fixture::trace();

    assert_eq!(
        none.hidden_from_agent_many(&owner, &docs, &trace).await,
        vec![false, false],
        "a person sees everything"
    );
    assert!(!none.hidden_from_agent(&owner, &first).await);
    assert_eq!(
        act_as(agent(), none.hidden_from_agent_many(&owner, &docs, &trace)).await,
        vec![true, true]
    );
    assert!(act_as(agent(), none.hidden_from_agent(&owner, &first)).await);
    assert_eq!(
        act_as(agent(), read.hidden_from_agent_many(&owner, &docs, &trace)).await,
        vec![false, false]
    );
    assert!(!act_as(agent(), read.hidden_from_agent(&owner, &second)).await);
}

#[tokio::test]
async fn a_subtree_splits_into_owned_trash_and_foreign_rehoming() {
    let h = rig().await;
    let owner = person("access-subtree-owner");
    let other = person("access-subtree-other");
    let mine = owned(&h, &owner).await;
    let theirs = owned(&h, &other).await;
    let service = access(&h, AgentCap::Edit, vec![AgentCapability::Trash]);
    let split = service
        .partition_subtree_documents(&owner, vec![mine.clone(), theirs.clone()])
        .await
        .expect("a person partitions freely");
    assert_eq!(
        split.trash.iter().map(|d| &d.id).collect::<Vec<_>>(),
        vec![&mine.id]
    );
    assert_eq!(
        split.rehome.iter().map(|d| &d.id).collect::<Vec<_>>(),
        vec![&theirs.id]
    );
    let allowed = act_as(
        agent(),
        service.partition_subtree_documents(&owner, vec![mine.clone(), theirs.clone()]),
    )
    .await
    .expect("the policy lets the agent trash");
    assert_eq!(allowed.trash.len(), 1);

    let strict = access(&h, AgentCap::Edit, Vec::new());
    let refused = act_as(
        agent(),
        strict.partition_subtree_documents(&owner, vec![mine.clone(), theirs]),
    )
    .await
    .expect_err("an agent trashes only what the policy allows");
    assert!(matches!(refused, DocumentsError::Agent(_)), "{refused:?}");
    let read_only = access(&h, AgentCap::Read, vec![AgentCapability::Trash]);
    act_as(
        agent(),
        read_only.partition_subtree_documents(&owner, vec![mine]),
    )
    .await
    .expect_err("a read cap never deletes");
}
