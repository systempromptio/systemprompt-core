//! What a policy is told: the creating agent and a nearest-first folder
//! chain; and the batch visibility check under an agent's cap.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::agents::{
    AgentAccessPolicy, AgentActor, AgentCap, DocumentContext,
};
use systemprompt_files::documents::service::NewFolderRequest;
use systemprompt_models::documents::EditOp;
use systemprompt_test_fixtures::test_db_pool;
use tempfile::TempDir;

use crate::documents_agent_fixture::{agent_harness, as_agent, document, edit};
use crate::documents_service_fixture::{
    GroupSource, RoleDirectory, allow, create, harness_with_policy, person, trace, unique,
};

/// Lets an agent create in any folder and edit only what it created itself,
/// and records the folder chains it was asked about.
#[derive(Default)]
struct OwnDrafts {
    seen: Mutex<Vec<Vec<String>>>,
}

#[async_trait]
impl AgentAccessPolicy for OwnDrafts {
    fn name(&self) -> &'static str {
        "own-drafts"
    }

    async fn cap(&self, doc: &DocumentContext, agent: &AgentActor) -> AgentCap {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(doc.folders.iter().map(ToString::to_string).collect());
        }
        let own = doc.created_by_agent.as_ref() == Some(&agent.name);
        if doc.document_id.is_none() || own {
            AgentCap::Edit
        } else {
            AgentCap::Read
        }
    }
}

#[tokio::test]
async fn the_policy_sees_the_creating_agent_and_a_nearest_first_chain() {
    let policy = Arc::new(OwnDrafts::default());
    let h = harness_with_policy(
        test_db_pool().await,
        (allow(), GroupSource::new(&[]), RoleDirectory::default()),
        Arc::new(TempDir::new().expect("tempdir")),
        DocumentsConfig::default(),
        Some(Arc::clone(&policy) as Arc<dyn AgentAccessPolicy>),
    );
    let owner = person("drafts-owner");
    let folder = |parent| NewFolderRequest {
        parent,
        name: unique("level"),
    };
    let top = h
        .service
        .create_folder(&owner, folder(None), &trace())
        .await
        .expect("top");
    let inner = h
        .service
        .create_folder(&owner, folder(Some(top.id.clone())), &trace())
        .await
        .expect("inner");

    let by_person = document(&h, &owner).await;
    as_agent(edit(&h, &owner, &by_person, "agent\n"))
        .await
        .expect_err("a person's document is read-only to the agent");

    let mut request = create("blank-text", Some(&inner.id));
    request.ops = vec![EditOp::SetText {
        text: "draft\n".to_owned(),
    }];
    let (draft, _) = as_agent(h.service.create_from_template(&owner, request, &trace()))
        .await
        .expect("the agent drafts");
    as_agent(edit(&h, &owner, &draft.id, "mine\n"))
        .await
        .expect("the agent edits its own draft");

    let seen = policy.seen.lock().expect("seen").clone();
    let nearest_first = vec![inner.id.to_string(), top.id.to_string()];
    assert!(seen.contains(&nearest_first), "{seen:?}");
}

#[tokio::test]
async fn the_batch_check_leaves_out_what_the_agent_cannot_see() {
    let h = agent_harness(
        AgentCap::None,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("batch-owner");
    let stranger = person("batch-stranger");
    let id = document(&h, &owner).await;
    let ids = vec![id.clone()];
    let mine = h
        .service
        .visible_documents(&owner, &ids, &trace())
        .await
        .expect("owner");
    assert_eq!(mine, ids);
    let theirs = h
        .service
        .visible_documents(&stranger, &ids, &trace())
        .await
        .expect("stranger");
    assert!(theirs.is_empty());
    let agents = as_agent(h.service.visible_documents(&owner, &ids, &trace()))
        .await
        .expect("agent");
    assert!(agents.is_empty());
}

#[tokio::test]
async fn a_suggest_capped_whole_file_write_becomes_a_proposal() {
    use systemprompt_identifiers::DriveDeviceId;
    use systemprompt_models::documents::{DriveCommitMeta, DriveCommitResponse, content_hash};

    let h = agent_harness(
        AgentCap::Suggest,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("suggest-drive-owner");
    let (document, revision) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let edited = b"# edited on a laptop\n".to_vec();
    let meta = DriveCommitMeta {
        document_id: Some(document.id.clone()),
        path: document.name.clone(),
        base_revision: Some(revision.id.clone()),
        content_sha256: content_hash(&edited),
        size_bytes: edited.len() as u64,
        device_id: DriveDeviceId::new("laptop"),
        message: None,
    };
    let response = as_agent(h.service.drive_commit(&owner, meta, edited, &trace()))
        .await
        .expect("a suggest-capped device write is proposed, not refused");
    assert!(
        matches!(response, DriveCommitResponse::Proposed { .. }),
        "{response:?}"
    );
    let head = h
        .service
        .get(&owner, &document.id, &trace())
        .await
        .expect("get")
        .head
        .map(|r| r.id);
    assert_eq!(head, Some(revision.id), "a proposal never moves the head");
}
