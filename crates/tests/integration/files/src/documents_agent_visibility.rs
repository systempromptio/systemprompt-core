//! What an agent and a person can see of caps: a folder capped `none`
//! never lists for an agent, and a folder the caller holds no grant on is
//! never named in a cap's source or reason.

use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::agents::{AgentCap, CapSource};
use systemprompt_files::documents::models::{GrantTarget, ListScope, ListSort};
use systemprompt_files::documents::service::{
    AgentCapTarget, ListFolderRequest, NewFolderRequest, ShareRequest,
};
use systemprompt_models::documents::AccessLevel;

use crate::documents_agent_fixture::{agent_harness, as_agent};
use crate::documents_service_fixture::{RoleDirectory, create, person, trace, unique};

#[tokio::test]
async fn a_none_folder_never_lists_for_an_agent() {
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("none-folder-owner");
    let hidden = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("hidden"),
            },
            &trace(),
        )
        .await
        .expect("folder");
    h.service
        .set_agent_cap(
            &owner,
            AgentCapTarget::Folder(hidden.id.clone()),
            Some(AgentCap::None),
            &trace(),
        )
        .await
        .expect("cap");
    let root = ListFolderRequest {
        scope: ListScope::Folder,
        folder: None,
        query: None,
        sort: ListSort::Name,
        cursor: None,
        limit: 200,
    };
    let mine = h
        .service
        .list_folder(&owner, root.clone(), &trace())
        .await
        .expect("owner");
    assert!(mine.folders.iter().any(|f| f.id == hidden.id));
    let agents = as_agent(h.service.list_folder(&owner, root, &trace()))
        .await
        .expect("agent");
    assert!(agents.folders.iter().all(|f| f.id != hidden.id));
}

#[tokio::test]
async fn a_folder_the_caller_cannot_view_is_never_named() {
    let owner = person("named-owner");
    let reader = person("named-reader");
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default().with_user(&reader.id, "Reader"),
        DocumentsConfig::default(),
    )
    .await;
    let folder = h
        .service
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: unique("private"),
            },
            &trace(),
        )
        .await
        .expect("folder");
    h.service
        .set_agent_cap(
            &owner,
            AgentCapTarget::Folder(folder.id.clone()),
            Some(AgentCap::Read),
            &trace(),
        )
        .await
        .expect("cap");
    let (document, _) = h
        .service
        .create_from_template(&owner, create("blank-text", Some(&folder.id)), &trace())
        .await
        .expect("create");
    h.service
        .share(
            &owner,
            ShareRequest {
                target: GrantTarget::Document(document.id.clone()),
                subject_kind: "user".to_owned(),
                subject_value: reader.id.as_str().to_owned(),
                subject_display: None,
                level: AccessLevel::View,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share the document only");

    let view = h
        .service
        .agent_cap(
            &reader,
            AgentCapTarget::Document(document.id.clone()),
            &trace(),
        )
        .await
        .expect("cap view");
    for cap in [&view.effective, &view.attended, &view.unattended] {
        assert_eq!(cap.cap, AgentCap::Read);
        assert_eq!(cap.source, CapSource::InheritedOverride);
        assert!(!cap.reason().contains(folder.id.as_str()));
    }
    let owners = h
        .service
        .agent_cap(
            &owner,
            AgentCapTarget::Document(document.id.clone()),
            &trace(),
        )
        .await
        .expect("owner view");
    assert!(matches!(
        owners.effective.source,
        CapSource::FolderOverride { .. }
    ));
}
