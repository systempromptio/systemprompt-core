//! A cap survives a copy or a move: a copy carries its source's cap, an
//! agent needs `copy_out` to copy a capped document, and moving into a
//! looser folder pins the cap the document had.

use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::agents::{AgentCap, AgentCapability, AgentRefusal};
use systemprompt_files::documents::service::{AgentCapTarget, CopyRequest, NewFolderRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;

use crate::documents_agent_fixture::{agent_harness, agent_refusal, as_agent, document};
use crate::documents_service_fixture::{Harness, RoleDirectory, api_author, person, trace, unique};

async fn own_cap(h: &Harness, owner: &AuthenticatedUser, id: &DocumentId) -> Option<AgentCap> {
    h.service
        .agent_cap(owner, AgentCapTarget::Document(id.clone()), &trace())
        .await
        .expect("cap")
        .own_override
}

fn copy_of(id: &DocumentId) -> CopyRequest {
    CopyRequest {
        source: id.clone(),
        name: None,
        folder: None,
        author: api_author(),
    }
}

#[tokio::test]
async fn a_copy_carries_the_cap_and_an_agent_needs_copy_out() {
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("copy-owner");
    let id = document(&h, &owner).await;
    h.service
        .set_agent_cap(
            &owner,
            AgentCapTarget::Document(id.clone()),
            Some(AgentCap::Read),
            &trace(),
        )
        .await
        .expect("cap the source");

    let (copied, _) = h
        .service
        .copy(&owner, copy_of(&id), &trace())
        .await
        .expect("the owner copies");
    assert_eq!(own_cap(&h, &owner, &copied.id).await, Some(AgentCap::Read));

    let refused = as_agent(h.service.copy(&owner, copy_of(&id), &trace()))
        .await
        .expect_err("an agent copying out of a capped document");
    assert!(matches!(
        agent_refusal(&refused),
        Some(AgentRefusal::HardRule {
            capability: AgentCapability::CopyOut,
            ..
        })
    ));
}

#[tokio::test]
async fn moving_into_a_looser_folder_pins_the_cap_it_had() {
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("move-owner");
    let folder = |name: &str| NewFolderRequest {
        parent: None,
        name: unique(name),
    };
    let strict = h
        .service
        .create_folder(&owner, folder("strict"), &trace())
        .await
        .expect("strict");
    let open = h
        .service
        .create_folder(&owner, folder("open"), &trace())
        .await
        .expect("open");
    h.service
        .set_agent_cap(
            &owner,
            AgentCapTarget::Folder(strict.id.clone()),
            Some(AgentCap::None),
            &trace(),
        )
        .await
        .expect("cap the folder");
    let id = document(&h, &owner).await;
    h.service
        .move_to(&owner, &id, Some(&strict.id), &trace())
        .await
        .expect("into the capped folder");
    assert_eq!(own_cap(&h, &owner, &id).await, None);
    h.service
        .move_to(&owner, &id, Some(&open.id), &trace())
        .await
        .expect("out to the open folder");
    assert_eq!(own_cap(&h, &owner, &id).await, Some(AgentCap::None));
    as_agent(h.service.get(&owner, &id, &trace()))
        .await
        .expect_err("still hidden from agents after the move");
}

/// Refuses every move into a folder named in `blocked`, and counts copies.
#[derive(Default)]
struct Hooks {
    blocked: std::sync::Mutex<Vec<String>>,
    copies: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl systemprompt_files::documents::agents::AgentAccessPolicy for Hooks {
    fn name(&self) -> &'static str {
        "hooks"
    }

    async fn cap(
        &self,
        _doc: &systemprompt_files::documents::agents::DocumentContext,
        _agent: &systemprompt_files::documents::agents::AgentActor,
    ) -> AgentCap {
        AgentCap::Edit
    }

    async fn on_move(
        &self,
        _doc: &systemprompt_files::documents::agents::DocumentContext,
        to: &[systemprompt_identifiers::DocumentFolderId],
        mover: &systemprompt_files::documents::agents::MoveActor,
    ) -> Result<(), systemprompt_files::documents::agents::MoveRefused> {
        let blocked = self.blocked.lock().map(|b| b.clone()).unwrap_or_default();
        match to.first() {
            Some(folder) if blocked.contains(&folder.to_string()) => {
                Err(systemprompt_files::documents::agents::MoveRefused {
                    reason: format!("{} may not move here", mover.user),
                })
            },
            _ => Ok(()),
        }
    }

    async fn on_copy(
        &self,
        _source: &systemprompt_files::documents::agents::DocumentContext,
        _copy: &DocumentId,
    ) {
        self.copies
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[tokio::test]
async fn the_policy_can_refuse_a_move_and_sees_every_copy() {
    let hooks = std::sync::Arc::new(Hooks::default());
    let h = crate::documents_service_fixture::harness_with_policy(
        systemprompt_test_fixtures::test_db_pool().await,
        (
            crate::documents_service_fixture::allow(),
            crate::documents_service_fixture::GroupSource::new(&[]),
            RoleDirectory::default(),
        ),
        std::sync::Arc::new(tempfile::TempDir::new().expect("tempdir")),
        DocumentsConfig::default(),
        Some(std::sync::Arc::clone(&hooks)
            as std::sync::Arc<
                dyn systemprompt_files::documents::agents::AgentAccessPolicy,
            >),
    );
    let owner = person("hook-owner");
    let fenced = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("fenced"),
            },
            &trace(),
        )
        .await
        .expect("folder");
    if let Ok(mut blocked) = hooks.blocked.lock() {
        blocked.push(fenced.id.to_string());
    }
    let id = document(&h, &owner).await;
    let refused = h
        .service
        .move_to(&owner, &id, Some(&fenced.id), &trace())
        .await
        .expect_err("the policy refuses the move");
    assert!(matches!(
        agent_refusal(&refused),
        Some(AgentRefusal::MoveRefused { .. })
    ));
    h.service
        .copy(&owner, copy_of(&id), &trace())
        .await
        .expect("copy");
    assert_eq!(hooks.copies.load(std::sync::atomic::Ordering::SeqCst), 1);
}
