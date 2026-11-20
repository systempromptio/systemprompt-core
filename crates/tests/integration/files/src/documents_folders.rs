use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::service::{FolderMove, FolderUpdateRequest, NewFolderRequest};
use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AgentEditPolicy, EditOp};

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, deny, harness, harness_on, person, trace, unique,
};

async fn folder(
    h: &Harness,
    who: &AuthenticatedUser,
    parent: Option<&DocumentFolderId>,
) -> DocumentFolderId {
    h.service
        .create_folder(
            who,
            NewFolderRequest {
                parent: parent.cloned(),
                name: unique("Folder"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id
}

async fn document_in(h: &Harness, who: &AuthenticatedUser, at: &DocumentFolderId) -> DocumentId {
    let mut request = create("blank-text", Some(at));
    request.name = unique("doc");
    request.ops = vec![EditOp::SetText {
        text: "body\n".to_owned(),
    }];
    h.service
        .create_from_template(who, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

#[tokio::test]
async fn rename_folder_updates_the_name_and_refuses_bad_names() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-rename");
    let id = folder(&h, &owner, None).await;
    let name = unique("Renamed");
    let renamed = h
        .service
        .rename_folder(&owner, &id, &name, &trace())
        .await
        .expect("rename");
    assert_eq!(renamed.name, name);
    assert_eq!(renamed.id, id);
    let bad = h
        .service
        .rename_folder(&owner, &id, "a/b", &trace())
        .await
        .expect_err("invalid");
    assert!(matches!(bad, DocumentsError::InvalidName { .. }));
}

#[tokio::test]
async fn a_folder_name_a_sibling_holds_is_refused_as_taken() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-taken");
    let parent = folder(&h, &owner, None).await;
    let name = unique("Taken");
    let create_named = |name: String, parent: Option<DocumentFolderId>| {
        let h = &h;
        let owner = &owner;
        async move {
            h.service
                .create_folder(owner, NewFolderRequest { parent, name }, &trace())
                .await
        }
    };
    let first = create_named(name.clone(), Some(parent.clone()))
        .await
        .expect("first")
        .id;
    let again = create_named(name.clone(), Some(parent.clone()))
        .await
        .expect_err("same name, same parent");
    assert!(
        matches!(&again, DocumentsError::FolderNameTaken { name: taken } if *taken == name),
        "{again:?}"
    );
    assert!(again.to_string().contains("folder"), "{again}");

    let sibling = folder(&h, &owner, Some(&parent)).await;
    let renamed = h
        .service
        .rename_folder(&owner, &sibling, &name, &trace())
        .await
        .expect_err("rename onto a sibling's name");
    assert!(
        matches!(renamed, DocumentsError::FolderNameTaken { .. }),
        "{renamed:?}"
    );
    let updated = h
        .service
        .update_folder(
            &owner,
            &sibling,
            FolderUpdateRequest {
                name: Some(name.clone()),
                ..FolderUpdateRequest::default()
            },
            &trace(),
        )
        .await
        .expect_err("update onto a sibling's name");
    assert!(
        matches!(updated, DocumentsError::FolderNameTaken { .. }),
        "{updated:?}"
    );

    let elsewhere = create_named(name.clone(), None)
        .await
        .expect("same name at the root")
        .id;
    let moved = h
        .service
        .move_folder(&owner, &elsewhere, Some(&parent), &trace())
        .await
        .expect_err("move beside a folder of the same name");
    assert!(
        matches!(&moved, DocumentsError::FolderNameTaken { name: taken } if *taken == name),
        "{moved:?}"
    );
    let moved_by_update = h
        .service
        .update_folder(
            &owner,
            &elsewhere,
            FolderUpdateRequest {
                parent: FolderMove::Into(parent.clone()),
                ..FolderUpdateRequest::default()
            },
            &trace(),
        )
        .await
        .expect_err("update moving beside a folder of the same name");
    assert!(
        matches!(moved_by_update, DocumentsError::FolderNameTaken { .. }),
        "{moved_by_update:?}"
    );
    let kept = h
        .repos
        .folders
        .find(&first)
        .await
        .expect("find")
        .expect("first kept");
    assert_eq!(kept.name, name);
}

#[tokio::test]
async fn move_folder_reparents_and_refuses_cycles() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-move");
    let parent = folder(&h, &owner, None).await;
    let child = folder(&h, &owner, None).await;
    let moved = h
        .service
        .move_folder(&owner, &child, Some(&parent), &trace())
        .await
        .expect("move");
    assert_eq!(moved.parent_id.as_ref(), Some(&parent));
    let into_self = h
        .service
        .move_folder(&owner, &child, Some(&child), &trace())
        .await
        .expect_err("self");
    assert!(matches!(into_self, DocumentsError::InvalidMove { .. }));
    let into_descendant = h
        .service
        .move_folder(&owner, &parent, Some(&child), &trace())
        .await
        .expect_err("cycle");
    assert!(matches!(
        into_descendant,
        DocumentsError::InvalidMove { .. }
    ));
    let to_root = h
        .service
        .move_folder(&owner, &child, None, &trace())
        .await
        .expect("root");
    assert!(to_root.parent_id.is_none());
}

#[tokio::test]
async fn delete_folder_refuses_non_empty_unless_recursive() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-delete");
    let root = folder(&h, &owner, None).await;
    let sub = folder(&h, &owner, Some(&root)).await;
    let doc = document_in(&h, &owner, &sub).await;
    let refused = h
        .service
        .delete_folder(&owner, &root, false, &trace())
        .await
        .expect_err("not empty");
    assert!(matches!(refused, DocumentsError::FolderNotEmpty { .. }));
    let with_doc = h
        .service
        .delete_folder(&owner, &sub, false, &trace())
        .await
        .expect_err("holds a document");
    assert!(matches!(with_doc, DocumentsError::FolderNotEmpty { .. }));

    h.service
        .delete_folder(&owner, &root, true, &trace())
        .await
        .expect("recursive");
    for id in [&root, &sub] {
        assert!(h.repos.folders.find(id).await.expect("find").is_none());
    }
    assert!(h.repos.documents.find(&doc).await.expect("find").is_none());
    let gone = h
        .service
        .rename_folder(&owner, &root, "After", &trace())
        .await
        .expect_err("deleted");
    assert!(!matches!(gone, DocumentsError::InvalidName { .. }));
}

#[tokio::test]
async fn empty_folder_deletes_without_recursion() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-empty");
    let id = folder(&h, &owner, None).await;
    h.service
        .delete_folder(&owner, &id, false, &trace())
        .await
        .expect("delete");
    assert!(h.repos.folders.find(&id).await.expect("find").is_none());
}

#[tokio::test]
async fn deny_hook_refuses_folder_mutations() {
    let open = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-deny");
    let id = folder(&open, &owner, None).await;
    let closed = harness_on(deny(), GroupSource::new(&[]), open.storage.clone()).await;
    let stranger = person("folder-stranger");
    let rename = closed
        .service
        .rename_folder(&stranger, &id, "Nope", &trace())
        .await
        .expect_err("rename");
    assert!(
        matches!(
            rename,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{rename:?}"
    );
    let moved = closed
        .service
        .move_folder(&stranger, &id, None, &trace())
        .await
        .expect_err("move");
    assert!(
        matches!(
            moved,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{moved:?}"
    );
    let deleted = closed
        .service
        .delete_folder(&stranger, &id, true, &trace())
        .await
        .expect_err("delete");
    assert!(
        matches!(
            deleted,
            DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }
        ),
        "{deleted:?}"
    );
    let row = open
        .repos
        .folders
        .find(&id)
        .await
        .expect("find")
        .expect("row");
    assert!(row.trashed_at.is_none());
    assert_ne!(row.name, "Nope");
}

#[tokio::test]
async fn update_folder_moves_renames_and_sets_the_policy_together() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-update");
    let parent = folder(&h, &owner, None).await;
    let child = folder(&h, &owner, None).await;
    let name = unique("Updated");
    let updated = h
        .service
        .update_folder(
            &owner,
            &child,
            FolderUpdateRequest {
                name: Some(name.clone()),
                parent: FolderMove::Into(parent.clone()),
                agent_edit_policy: Some(AgentEditPolicy::Propose),
            },
            &trace(),
        )
        .await
        .expect("update");
    assert_eq!(updated.name, name);
    assert_eq!(updated.parent_id.as_ref(), Some(&parent));
    assert_eq!(updated.agent_edit_policy, AgentEditPolicy::Propose);
}

#[tokio::test]
async fn update_folder_that_cannot_move_leaves_name_and_policy_unchanged() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("folder-update-atomic");
    let parent = folder(&h, &owner, None).await;
    let child = folder(&h, &owner, Some(&parent)).await;
    let before = h
        .service
        .update_folder(
            &owner,
            &parent,
            FolderUpdateRequest {
                name: None,
                parent: FolderMove::Keep,
                agent_edit_policy: Some(AgentEditPolicy::Direct),
            },
            &trace(),
        )
        .await
        .expect("baseline");
    let refused = h
        .service
        .update_folder(
            &owner,
            &parent,
            FolderUpdateRequest {
                name: Some(unique("Never")),
                parent: FolderMove::Into(child),
                agent_edit_policy: Some(AgentEditPolicy::Propose),
            },
            &trace(),
        )
        .await
        .expect_err("a folder cannot move under its own descendant");
    assert!(matches!(refused, DocumentsError::InvalidMove { .. }));
    let after = h
        .service
        .update_folder(
            &owner,
            &parent,
            FolderUpdateRequest {
                name: None,
                parent: FolderMove::Keep,
                agent_edit_policy: Some(AgentEditPolicy::Direct),
            },
            &trace(),
        )
        .await
        .expect("read back");
    assert_eq!(after.name, before.name);
    assert_eq!(after.agent_edit_policy, AgentEditPolicy::Direct);
}
