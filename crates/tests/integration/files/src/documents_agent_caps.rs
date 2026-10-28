//! Agent caps: the matrix of what each cap leaves an agent acting for the
//! owner, invisibility under `none`, `suggest` turning an edit into a
//! proposal, and the owner's override that only tightens.

use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::{AgentCap, AgentRefusal};
use systemprompt_files::documents::models::{ListScope, ListSort};
use systemprompt_files::documents::service::{
    AgentCapTarget, CommentRequest, EditModeRequest, ListFolderRequest, NewFolderRequest,
};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::{ActorKind, EditMode};

use crate::documents_agent_fixture::{agent_harness, agent_refusal, as_agent, document, edit};
use crate::documents_service_fixture::{RoleDirectory, create, person, trace, unique};

fn comment_on(id: &DocumentId) -> CommentRequest {
    CommentRequest {
        document: id.clone(),
        anchor: None,
        range: None,
        body: "looks right".to_owned(),
        mentions: Vec::new(),
        parent_id: None,
    }
}

#[tokio::test]
async fn each_cap_leaves_the_agent_what_it_names_and_a_person_is_never_capped() {
    for cap in AgentCap::ALL {
        let h = agent_harness(
            cap,
            Vec::new(),
            RoleDirectory::default(),
            DocumentsConfig::default(),
        )
        .await;
        let owner = person("cap-owner");
        let id = document(&h, &owner).await;

        let read = as_agent(h.service.get(&owner, &id, &trace())).await;
        let comment = as_agent(h.service.create_comment(&owner, comment_on(&id), &trace())).await;
        let wrote = as_agent(edit(&h, &owner, &id, "agent\n")).await;
        match cap {
            AgentCap::None => {
                assert!(matches!(read, Err(DocumentsError::NotFound { .. })));
                assert!(matches!(comment, Err(DocumentsError::NotFound { .. })));
                assert!(matches!(wrote, Err(DocumentsError::NotFound { .. })));
            },
            AgentCap::Read => {
                assert!(read.is_ok());
                let refused = comment.expect_err("read refuses comment");
                assert!(matches!(
                    agent_refusal(&refused),
                    Some(AgentRefusal::CapExceeded { .. })
                ));
                assert!(wrote.is_err());
            },
            AgentCap::Comment => {
                assert!(read.is_ok() && comment.is_ok());
                let refused = wrote.expect_err("comment refuses edit");
                assert!(matches!(
                    agent_refusal(&refused),
                    Some(AgentRefusal::CapExceeded { .. })
                ));
            },
            AgentCap::Suggest => {
                assert!(read.is_ok() && comment.is_ok());
                let refused = wrote.expect_err("suggest refuses a direct edit");
                assert!(matches!(
                    agent_refusal(&refused),
                    Some(AgentRefusal::SuggestOnly { .. })
                ));
            },
            AgentCap::Edit => {
                assert!(read.is_ok() && comment.is_ok() && wrote.is_ok());
            },
        }
        edit(&h, &owner, &id, "the owner, directly\n")
            .await
            .expect("a person is never capped");
    }
}

#[tokio::test]
async fn under_none_the_document_is_missing_from_an_agents_listing() {
    let h = agent_harness(
        AgentCap::None,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("hidden-owner");
    let id = document(&h, &owner).await;
    let request = ListFolderRequest {
        scope: ListScope::Mine,
        folder: None,
        query: None,
        sort: ListSort::Updated,
        cursor: None,
        limit: 50,
    };
    let mine = h
        .service
        .list_folder(&owner, request.clone(), &trace())
        .await
        .expect("owner lists");
    assert!(mine.documents.iter().any(|d| d.document.id == id));
    let agents = as_agent(h.service.list_folder(&owner, request, &trace()))
        .await
        .expect("agent lists");
    assert!(agents.documents.iter().all(|d| d.document.id != id));
}

#[tokio::test]
async fn under_suggest_an_edit_that_asked_to_land_becomes_a_proposal() {
    let h = agent_harness(
        AgentCap::Suggest,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("suggest-owner");
    let id = document(&h, &owner).await;
    let mode = as_agent(h.service.edit_mode(
        &owner,
        EditModeRequest {
            document: &id,
            requested: Some(EditMode::Land),
            actor: ActorKind::Agent,
        },
        &trace(),
    ))
    .await
    .expect("mode");
    assert_eq!(mode, EditMode::Propose);
}

#[tokio::test]
async fn an_override_only_tightens_and_an_agent_cannot_set_one() {
    let h = agent_harness(
        AgentCap::Comment,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("override-owner");
    let folder = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("capped"),
            },
            &trace(),
        )
        .await
        .expect("folder");
    let request = create("blank-text", Some(&folder.id));
    let (inside, _) = h
        .service
        .create_from_template(&owner, request, &trace())
        .await
        .expect("create in folder");

    let looser = h
        .service
        .set_agent_cap(
            &owner,
            AgentCapTarget::Folder(folder.id.clone()),
            Some(AgentCap::Edit),
            &trace(),
        )
        .await
        .expect_err("looser than the policy");
    assert!(matches!(
        agent_refusal(&looser),
        Some(AgentRefusal::OverrideLooser { .. })
    ));

    let view = h
        .service
        .set_agent_cap(
            &owner,
            AgentCapTarget::Folder(folder.id.clone()),
            Some(AgentCap::Read),
            &trace(),
        )
        .await
        .expect("tighten");
    assert_eq!(view.own_override, Some(AgentCap::Read));
    assert_eq!(view.effective.cap, AgentCap::Read);

    let refused = as_agent(
        h.service
            .create_comment(&owner, comment_on(&inside.id), &trace()),
    )
    .await
    .expect_err("the folder override reaches the document inside");
    assert!(matches!(
        agent_refusal(&refused),
        Some(AgentRefusal::CapExceeded {
            cap: AgentCap::Read,
            ..
        })
    ));

    let by_agent = as_agent(h.service.set_agent_cap(
        &owner,
        AgentCapTarget::Document(inside.id.clone()),
        None,
        &trace(),
    ))
    .await
    .expect_err("an agent cannot move its own cap");
    assert!(matches!(
        agent_refusal(&by_agent),
        Some(AgentRefusal::HardRule { .. })
    ));
}
